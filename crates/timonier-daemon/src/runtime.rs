//! Where a project's daemon lives (ADR 0019): its socket and lock in a private runtime directory,
//! its database in the repository's `.timonier/`.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Overrides the runtime directory: tests, or a session without `$XDG_RUNTIME_DIR`.
pub const RUNTIME_DIR_VAR: &str = "TIMONIER_RUNTIME_DIR";

/// What `.timonier/.gitignore` keeps out of git: the database and its WAL files, a rebuildable
/// cache, and the reports of the test runs (ADR 0034).
pub const GITIGNORE: &str = "# Written by Timonier: the graph is rebuilt from the repository, and test reports are\n# not versioned.\ngraph.sqlite*\nruns/\n";

/// The files of one project's daemon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    /// The repository, canonical.
    pub root: PathBuf,
    pub runtime: PathBuf,
    pub socket: PathBuf,
    pub lock: PathBuf,
    /// Where a daemon started in the background writes its logs (`timonier daemon start`).
    pub log: PathBuf,
    pub database: PathBuf,
}

impl Paths {
    /// The paths of the daemon of the repository at `root`, in the default runtime directory.
    pub fn new(root: &Path) -> Result<Self> {
        Self::in_runtime(root, &runtime_dir())
    }

    pub fn in_runtime(root: &Path, runtime: &Path) -> Result<Self> {
        let root = root.canonicalize().map_err(|source| Error::Io {
            path: root.to_path_buf(),
            source,
        })?;
        if not_a_project(&root) {
            return Err(Error::NotAProject(root));
        }
        // A short name: the 104 bytes of a Unix socket path on macOS (ADR 0019).
        let hash = blake3::hash(root.as_os_str().as_bytes()).to_hex();
        let name = &hash.as_str()[..16];
        Ok(Self {
            database: root.join(".timonier").join("graph.sqlite"),
            socket: runtime.join(format!("{name}.sock")),
            lock: runtime.join(format!("{name}.lock")),
            log: runtime.join(format!("{name}.log")),
            runtime: runtime.to_path_buf(),
            root,
        })
    }
}

/// `$TIMONIER_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR/timonier` (Linux), else `$TMPDIR/timonier`
/// (macOS, a directory of the user's own), else `timonier/run` in the user's cache directory. A
/// shared `/tmp/timonier-<uid>` is avoided: another user could create it first and block the
/// daemon.
pub fn runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(RUNTIME_DIR_VAR).filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if cfg!(target_os = "linux")
        && let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty())
    {
        return PathBuf::from(dir).join("timonier");
    }
    if cfg!(target_os = "macos") {
        return std::env::temp_dir().join("timonier");
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|d| !d.is_empty())
                .map(|home| PathBuf::from(home).join(".cache"))
        });
    match cache {
        Some(cache) => cache.join("timonier").join("run"),
        // No home at all: the temporary directory, checked like any other (owner, mode 0700).
        None => match std::fs::metadata("/proc/self") {
            Ok(meta) => std::env::temp_dir().join(format!("timonier-{}", meta.uid())),
            Err(_) => std::env::temp_dir().join("timonier"),
        },
    }
}

/// The daemon's lock, held for its whole life: one daemon per project.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

impl Lock {
    /// Takes the lock, creating the runtime directory. `None`: another daemon holds it.
    pub fn acquire(paths: &Paths) -> Result<Option<Self>> {
        let file = open_lock(paths)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(source)) => Err(Error::Io {
                path: paths.lock.clone(),
                source,
            }),
        }
    }

    /// Whether a daemon holds the lock now. A stale socket is not a daemon.
    pub fn is_held(paths: &Paths) -> Result<bool> {
        if !paths.lock.exists() {
            return Ok(false);
        }
        // Taken, then released at once when the file closes.
        Ok(Self::acquire(paths)?.is_none())
    }
}

/// Creates the runtime directory, closed to others, or checks the one there.
pub fn prepare_runtime(paths: &Paths) -> Result<()> {
    open_lock(paths).map(drop)
}

fn open_lock(paths: &Paths) -> Result<File> {
    let io = |source| Error::Io {
        path: paths.runtime.clone(),
        source,
    };
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&paths.runtime)
        .map_err(io)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&paths.lock)
        .map_err(|source| Error::Io {
            path: paths.lock.clone(),
            source,
        })?;
    // Only its user may reach the socket: the directory must be theirs, closed to others.
    // The lock file was created by this process, so it carries the user's id.
    let dir = std::fs::metadata(&paths.runtime).map_err(io)?;
    let mine = file.metadata().map_err(io)?.uid();
    if dir.uid() != mine || dir.permissions().mode() & 0o077 != 0 {
        return Err(Error::RuntimeDir {
            path: paths.runtime.clone(),
        });
    }
    Ok(file)
}

/// Creates `.timonier/` and its `.gitignore` when they are missing (spec « Initialisation »).
/// An existing `.gitignore` is the user's: it is left as it is.
pub fn prepare_repository(paths: &Paths) -> Result<()> {
    let dir = paths.root.join(".timonier");
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
        path: dir.clone(),
        source,
    })?;
    let gitignore = dir.join(".gitignore");
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&gitignore)
    {
        Ok(mut file) => file
            .write_all(GITIGNORE.as_bytes())
            .map_err(|source| Error::Io {
                path: gitignore,
                source,
            }),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(source) => Err(Error::Io {
            path: gitignore,
            source,
        }),
    }
}

/// The home folder and `/`: a daemon there would watch and index everything under it (T-119).
pub fn not_a_project(root: &Path) -> bool {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .and_then(|h| Path::new(&h).canonicalize().ok());
    root.parent().is_none() || home.as_deref() == Some(root)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_file_system_s_root_is_no_project() {
        assert!(matches!(
            super::Paths::in_runtime(std::path::Path::new("/"), std::path::Path::new("/tmp")),
            Err(crate::Error::NotAProject(_))
        ));
        assert!(!super::not_a_project(&std::env::temp_dir().join("x")));
    }

    use super::*;

    #[test]
    fn a_project_has_short_stable_names() {
        let repo = tempfile::tempdir().unwrap();
        let runtime = tempfile::tempdir().unwrap();
        let a = Paths::in_runtime(repo.path(), runtime.path()).unwrap();
        // Another spelling of the same directory names the same daemon.
        let dotted = repo.path().join(".");
        let b = Paths::in_runtime(&dotted, runtime.path()).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a.socket.file_name().unwrap().len(),
            "0123456789abcdef.sock".len()
        );
        assert_eq!(a.database, a.root.join(".timonier/graph.sqlite"));
        let other = tempfile::tempdir().unwrap();
        let c = Paths::in_runtime(other.path(), runtime.path()).unwrap();
        assert_ne!(a.socket, c.socket);
    }

    #[test]
    fn one_daemon_holds_the_lock() {
        let repo = tempfile::tempdir().unwrap();
        let runtime = tempfile::tempdir().unwrap();
        let paths = Paths::in_runtime(repo.path(), &runtime.path().join("timonier")).unwrap();
        assert!(!Lock::is_held(&paths).unwrap());
        let lock = Lock::acquire(&paths).unwrap().unwrap();
        let mode = std::fs::metadata(&paths.runtime)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
        assert!(Lock::acquire(&paths).unwrap().is_none());
        assert!(Lock::is_held(&paths).unwrap());
        drop(lock);
        // A process another test forks holds the lock's file until it executes its program
        // (review n° 8, m8-4), and `is_held` takes the lock a moment, which such a process may
        // keep too (n° 11, m11-6): free once it has.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while Lock::is_held(&paths).unwrap() {
            assert!(std::time::Instant::now() < deadline, "the lock stays held");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let again = loop {
            if let Some(lock) = Lock::acquire(&paths).unwrap() {
                break lock;
            }
            assert!(std::time::Instant::now() < deadline, "the lock stays held");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        drop(again);
    }

    #[test]
    fn a_runtime_directory_open_to_others_is_refused() {
        let repo = tempfile::tempdir().unwrap();
        let runtime = tempfile::tempdir().unwrap();
        let dir = runtime.path().join("open");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        let paths = Paths::in_runtime(repo.path(), &dir).unwrap();
        assert!(matches!(
            Lock::acquire(&paths),
            Err(Error::RuntimeDir { .. })
        ));
    }

    #[test]
    fn the_repository_gets_its_gitignore_once() {
        let repo = tempfile::tempdir().unwrap();
        let runtime = tempfile::tempdir().unwrap();
        let paths = Paths::in_runtime(repo.path(), runtime.path()).unwrap();
        prepare_repository(&paths).unwrap();
        let gitignore = paths.root.join(".timonier/.gitignore");
        assert!(
            std::fs::read_to_string(&gitignore)
                .unwrap()
                .contains("graph.sqlite*")
        );
        std::fs::write(&gitignore, "mine\n").unwrap();
        prepare_repository(&paths).unwrap();
        assert_eq!(std::fs::read_to_string(&gitignore).unwrap(), "mine\n");
    }
}
