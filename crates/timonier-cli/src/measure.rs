//! `timonier __measure`: the measuring relay of `timonier test` (ADR 0045, 0046). A small
//! process whose only child is the command: it counts its instructions, runs it, waits for it,
//! and writes what it measured of the whole tree to a file.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use clap::Args;
use timonier_core::perf::{Accounting, Measured, Sample};

#[derive(Debug, Args)]
pub struct MeasureArgs {
    /// Where to write what was measured, as JSON
    #[arg(long)]
    out: PathBuf,
    /// The relay runs in a transient systemd scope of its own: its cgroup holds the tree
    #[arg(long)]
    scope: bool,
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

/// How often the scope's resources are sampled, for the curve under the timeline.
const SAMPLING: Duration = Duration::from_millis(100);

pub fn run(args: &MeasureArgs) -> ExitCode {
    let cgroup = args.scope.then(own_cgroup).flatten();
    let mut counter = counter::open();
    let start = Instant::now();
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    let mut child = match Command::new(&args.command[0])
        .args(&args.command[1..])
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("timonier: cannot run `{}`: {error}", args.command[0]);
            return ExitCode::from(127);
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    let sampler = cgroup.clone().map(|cgroup| {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || sample(&cgroup, start, &stop))
    });
    let status = child.wait();
    let wall = start.elapsed();
    stop.store(true, Ordering::SeqCst);
    let samples = sampler.and_then(|s| s.join().ok()).unwrap_or_default();
    let (instructions, counter_refused) = match &mut counter {
        Ok(counter) => match counter::read(counter) {
            Ok(count) => (Some(count), None),
            Err(why) => (None, Some(why)),
        },
        Err(why) => (None, Some(why.clone())),
    };
    let usage = children_usage();
    let scope_cpu = cgroup.as_deref().and_then(scope_cpu_us);
    let exit_code = status.as_ref().ok().and_then(ExitStatus::code);
    let (io_read_bytes, io_write_bytes) = cgroup.as_deref().map_or((None, None), io_bytes);
    let measured = Measured {
        wall_us: u64::try_from(wall.as_micros()).unwrap_or(u64::MAX),
        exit_code,
        accounting: if cgroup.is_some() {
            Accounting::SystemdScope
        } else if usage.is_some() {
            Accounting::Rusage
        } else {
            Accounting::WallTime
        },
        user_cpu_us: scope_cpu
            .map(|(user, _)| user)
            .or_else(|| usage.map(|u| u.user_us))
            .unwrap_or(0),
        system_cpu_us: scope_cpu
            .map(|(_, system)| system)
            .or_else(|| usage.map(|u| u.system_us))
            .unwrap_or(0),
        largest_process_bytes: usage.map_or(0, |u| u.max_rss_bytes),
        tree_peak_bytes: cgroup
            .as_deref()
            .and_then(|c| number(&c.join("memory.peak"))),
        block_reads: usage.map_or(0, |u| u.block_reads),
        block_writes: usage.map_or(0, |u| u.block_writes),
        io_read_bytes,
        io_write_bytes,
        instructions,
        counter_refused,
        samples,
        started_at_ms,
    };
    if let Err(error) = std::fs::write(&args.out, serde_json::to_vec(&measured).unwrap_or_default())
    {
        eprintln!("timonier: cannot write the measures: {error}");
    }
    match status {
        Ok(status) => ExitCode::from(exit_byte(&status)),
        Err(_) => ExitCode::FAILURE,
    }
}

/// What the waited-for children used, as `getrusage` counts it.
#[derive(Clone, Copy)]
struct Usage {
    user_us: u64,
    system_us: u64,
    max_rss_bytes: u64,
    block_reads: u64,
    block_writes: u64,
}

#[cfg(unix)]
fn children_usage() -> Option<Usage> {
    use nix::sys::resource::{UsageWho, getrusage};
    use nix::sys::time::TimeValLike;
    let usage = getrusage(UsageWho::RUSAGE_CHILDREN).ok()?;
    let micros = |t: nix::sys::time::TimeVal| u64::try_from(t.num_microseconds()).unwrap_or(0);
    let count = |v: nix::libc::c_long| u64::try_from(v).unwrap_or(0);
    Some(Usage {
        user_us: micros(usage.user_time()),
        system_us: micros(usage.system_time()),
        max_rss_bytes: max_rss_bytes(count(usage.max_rss())),
        block_reads: count(usage.block_reads()),
        block_writes: count(usage.block_writes()),
    })
}

/// Windows gives the CPU and memory of a tree only through calls the workspace does not make
/// (no `unsafe`): the run says « not measured » (T-125).
#[cfg(not(unix))]
fn children_usage() -> Option<Usage> {
    None
}

/// `ru_maxrss` is in kibibytes on Linux, in bytes on macOS.
#[cfg(unix)]
fn max_rss_bytes(value: u64) -> u64 {
    if cfg!(target_os = "macos") {
        value
    } else {
        value.saturating_mul(1024)
    }
}

/// The exit status the relay gives back: the command's, or 128 + its signal, as a shell does.
#[cfg(not(unix))]
fn exit_byte(status: &ExitStatus) -> u8 {
    status
        .code()
        .map_or(1, |code| u8::try_from(code & 0xff).unwrap_or(1))
}

#[cfg(unix)]
fn exit_byte(status: &ExitStatus) -> u8 {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => u8::try_from(code & 0xff).unwrap_or(1),
        (None, Some(signal)) => u8::try_from((128 + signal) & 0xff).unwrap_or(1),
        (None, None) => 1,
    }
}

/// The relay's own cgroup v2, when it has the files a scope gives.
fn own_cgroup() -> Option<PathBuf> {
    let text = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path = text.lines().find_map(|line| line.strip_prefix("0::"))?;
    let dir = Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    dir.join("memory.peak").exists().then_some(dir)
}

fn number(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// `usage_usec` of a cgroup's `cpu.stat`.
fn cpu_us(cgroup: &Path) -> Option<u64> {
    cpu_field(cgroup, "usage_usec")
}

/// User and system time of the whole scope, detached descendants included (relecture n° 1,
/// m8): `getrusage` counts only the children waited for.
fn scope_cpu_us(cgroup: &Path) -> Option<(u64, u64)> {
    Some((
        cpu_field(cgroup, "user_usec")?,
        cpu_field(cgroup, "system_usec")?,
    ))
}

fn cpu_field(cgroup: &Path, field: &str) -> Option<u64> {
    let text = std::fs::read_to_string(cgroup.join("cpu.stat")).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix(field)?.strip_prefix(' '))
        .and_then(|v| v.trim().parse().ok())
}

/// The bytes read and written, summed over the devices of `io.stat`, when io is delegated.
fn io_bytes(cgroup: &Path) -> (Option<u64>, Option<u64>) {
    let Ok(text) = std::fs::read_to_string(cgroup.join("io.stat")) else {
        return (None, None);
    };
    let field = |name: &str| -> u64 {
        text.split_whitespace()
            .filter_map(|word| word.strip_prefix(name))
            .filter_map(|v| v.parse::<u64>().ok())
            .sum()
    };
    (Some(field("rbytes=")), Some(field("wbytes=")))
}

fn sample(cgroup: &Path, start: Instant, stop: &AtomicBool) -> Vec<Sample> {
    let mut samples = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(SAMPLING);
        if let (Some(memory_bytes), Some(cpu_us)) =
            (number(&cgroup.join("memory.current")), cpu_us(cgroup))
        {
            samples.push(Sample {
                at_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
                memory_bytes,
                cpu_us,
            });
        }
    }
    samples
}

/// The instructions counter (ADR 0046): Linux only, refused when the kernel says so.
#[cfg(target_os = "linux")]
mod counter {
    use perf_event::{Builder, Counter};

    pub fn open() -> Result<Counter, String> {
        let mut builder = Builder::new().observe_self();
        builder.inherit(true).exclude_kernel(true).exclude_hv(true);
        let mut counter = builder.build().map_err(refused)?;
        counter.enable().map_err(refused)?;
        Ok(counter)
    }

    pub fn read(counter: &mut Counter) -> Result<u64, String> {
        counter.read().map_err(|e| e.to_string())
    }

    fn refused(error: std::io::Error) -> String {
        let paranoid = std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid")
            .map(|t| t.trim().to_owned())
            .unwrap_or_else(|_| "?".to_owned());
        format!(
            "the kernel refuses the instruction counters ({error}; perf_event_paranoid is \
             {paranoid}): `timonier doctor` says how to allow them"
        )
    }
}

#[cfg(not(target_os = "linux"))]
mod counter {
    pub struct Counter;

    pub fn open() -> Result<Counter, String> {
        Err("no instruction counter without privileges on this system".to_owned())
    }

    pub fn read(_: &mut Counter) -> Result<u64, String> {
        Err("no instruction counter".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_io_bytes_add_up_over_the_devices() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("io.stat"),
            "8:0 rbytes=100 wbytes=20 rios=1 wios=1\n259:0 rbytes=5 wbytes=1 rios=1 wios=1\n",
        )
        .unwrap();
        assert_eq!(io_bytes(dir.path()), (Some(105), Some(21)));
        let none = tempfile::tempdir().unwrap();
        assert_eq!(io_bytes(none.path()), (None, None));
    }

    #[test]
    fn the_cpu_of_a_cgroup_is_its_usage() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("cpu.stat"),
            "usage_usec 1234\nuser_usec 1000\nsystem_usec 234\n",
        )
        .unwrap();
        assert_eq!(cpu_us(dir.path()), Some(1234));
        assert_eq!(scope_cpu_us(dir.path()), Some((1000, 234)));
        let none = tempfile::tempdir().unwrap();
        assert_eq!(scope_cpu_us(none.path()), None);
    }

    #[test]
    #[cfg(unix)]
    fn a_signal_gives_128_and_its_number() {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(exit_byte(&ExitStatus::from_raw(3 << 8)), 3);
        assert_eq!(exit_byte(&ExitStatus::from_raw(9)), 137);
    }
}
