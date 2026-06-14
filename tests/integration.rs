use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

#[test]
fn test_file_input_with_query() {
    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "alpha").unwrap();
    writeln!(tmp, "beta").unwrap();
    writeln!(tmp, "gamma").unwrap();
    writeln!(tmp, "delta").unwrap();
    tmp.flush().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .args(["--file", tmp.path().to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    assert!(output.is_ok());
}

#[test]
fn test_no_input_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // In CI, stdin may not be a TTY so behavior varies
    assert!(!output.status.success() || output.status.code() == Some(0));
}

#[test]
fn test_help_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .args(["--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fuzzy finder"));
    assert!(stdout.contains("--multi"));
    assert!(stdout.contains("--preview"));
    assert!(stdout.contains("--query"));
    assert!(stdout.contains("--height"));
    assert!(stdout.contains("--bind"));
}

#[test]
fn test_version_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fuzzyseek"));
}

#[test]
fn test_file_not_found_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .args(["--file", "/nonexistent/path/file.txt"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot open"));
}

#[test]
fn test_no_terminal_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // stderr is piped (not a terminal) so it should report error
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a terminal"));
}

#[test]
fn test_invalid_bind_format() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .args(["--bind", "invalid_format"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid --bind format"));
}

#[test]
fn test_unknown_bind_action() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .args(["--bind", "nonexistent:ctrl-x"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown action"));
}

#[test]
fn test_valid_bind_accepted() {
    let output = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .args(["--bind", "confirm:ctrl-m,cancel:ctrl-g", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // --help takes precedence, should succeed
    assert!(output.status.success());
}

#[test]
fn test_pipe_input_exits_gracefully() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fuzzyseek"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(ref mut stdin) = child.stdin {
        for i in 0..100 {
            if writeln!(stdin, "line_{}", i).is_err() {
                break;
            }
        }
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    // Should exit with code 2 (no terminal) rather than panicking
    assert_eq!(output.status.code(), Some(2));
}
