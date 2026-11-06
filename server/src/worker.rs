//! A disposable lexer process keeps a pathological regex from hanging the LSP.
use std::io::{self, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_OUTPUT: u64 = 2 * 1024 * 1024;

/// Reads one document from stdin and writes lexical token data to stdout.
pub fn run() -> io::Result<()> {
    let mut text = String::new();
    io::stdin()
        .take((super::MAX_DOCUMENT + 1) as u64)
        .read_to_string(&mut text)?;
    if text.len() > super::MAX_DOCUMENT {
        return Err(io::Error::other("document too large"));
    }
    serde_json::to_writer(io::stdout(), &super::highlight::semantic_tokens(&text))?;
    Ok(())
}

/// Runs a fixed lexer subprocess, killing and reaping it if its deadline expires.
pub fn tokens(text: &str) -> io::Result<Vec<u32>> {
    // The executable and switch are fixed; no URI, fence label, or Markdown text
    // becomes a path or command argument. No shell and no third-party plugins.
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--highlight-worker")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let text = text.to_owned();
    let (sender, receiver) = mpsc::sync_channel(1);
    // Pipe I/O must not block the thread enforcing the deadline.
    let io_thread = std::thread::spawn(move || {
        let result = (|| {
            stdin.write_all(text.as_bytes())?;
            drop(stdin);
            let mut output = Vec::new();
            stdout.take(MAX_OUTPUT + 1).read_to_end(&mut output)?;
            if output.len() as u64 > MAX_OUTPUT {
                return Err(io::Error::other("too many tokens"));
            }
            serde_json::from_slice(&output).map_err(io::Error::other)
        })();
        let _ = sender.send(result);
    });
    let status = wait(&mut child, TIMEOUT);
    // wait kills/reaps on timeout, closing both pipes before joining.
    let _ = io_thread.join();
    status?;
    receiver.recv().map_err(io::Error::other)?
}

fn wait(child: &mut Child, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err(io::Error::other("lexer failed")),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result.err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "lexer timed out")
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "subprocess fixture for deadline test"]
    fn slow_fixture() {
        std::thread::sleep(Duration::from_secs(30));
    }

    #[test]
    fn deadline_kills_and_reaps_worker() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "worker::tests::slow_fixture", "--ignored"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        assert_eq!(
            wait(&mut child, Duration::from_millis(50))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(child.try_wait().unwrap().is_some());
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
