use std::io;
use std::process::{Command, Output, Stdio};

/// Run a command with no inherited tty: stdin null, stdout/stderr piped.
/// `pm`/`cmd` fail with `Failed transaction` when any std fd is a pty.
pub fn run_capture(program: &str, args: &[&str]) -> io::Result<Output> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

/// `run_capture` returning stdout as a lossy string; non-zero exit is an error.
pub fn run_stdout(program: &str, args: &[&str]) -> Result<String, String> {
    let out = run_capture(program, args).map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err.lines().next().unwrap_or("").trim();
        return Err(format!("{program} exited with {}: {first}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
