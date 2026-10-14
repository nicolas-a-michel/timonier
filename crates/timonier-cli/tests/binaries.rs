//! `timonier` and `tim` are the same program under two names.

use assert_cmd::cargo::cargo_bin_cmd;

fn stdout_of(bin: &str, args: &[&str]) -> String {
    let output = match bin {
        "timonier" => cargo_bin_cmd!("timonier").args(args).output(),
        "tim" => cargo_bin_cmd!("tim").args(args).output(),
        other => panic!("unknown binary {other}"),
    }
    .expect("the binary should start");
    assert!(output.status.success(), "{bin} {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).expect("stdout should be UTF-8")
}

#[test]
fn both_names_print_the_same_version() {
    let expected = format!("timonier {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(stdout_of("timonier", &["--version"]), expected);
    assert_eq!(stdout_of("tim", &["--version"]), expected);
}

#[test]
fn help_describes_the_program_and_uses_the_invoked_name() {
    for bin in ["timonier", "tim"] {
        for args in [&["--help"][..], &[][..]] {
            let help = stdout_of(bin, args);
            assert!(help.contains("Steer AI coding agents"), "{help}");
            assert!(
                help.contains(&format!("Usage: {bin}")),
                "{bin} {args:?}: {help}"
            );
        }
    }
}

#[test]
fn an_unknown_argument_is_a_usage_error() {
    cargo_bin_cmd!("tim")
        .arg("--no-such-flag")
        .assert()
        .failure()
        .code(2);
}
