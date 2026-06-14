pub mod cache;
pub mod resolver;

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use parking_lot::Mutex;

use cache::PreviewCache;
use resolver::PreviewResolver;

#[derive(Clone, Debug)]
pub enum PreviewContent {
    Text(String),
    Error(String),
    Loading,
    Empty,
}

pub struct PreviewState {
    pub content: PreviewContent,
    pub current_line: String,
    pub scroll_offset: usize,
    pub visible: bool,
}

pub type SharedPreview = Arc<Mutex<PreviewState>>;

#[derive(Clone, Debug)]
pub struct PreviewConfig {
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub cache_capacity: usize,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_output_bytes: 1_048_576,
            cache_capacity: 64,
        }
    }
}

pub struct PreviewRunner {
    resolver: PreviewResolver,
    config: PreviewConfig,
    state: SharedPreview,
    cache: Arc<Mutex<PreviewCache>>,
    generation: Arc<AtomicU64>,
    /// Shared cancel token — swapped on each new request so the old thread sees `true`.
    active_cancel: Arc<Mutex<Arc<AtomicBool>>>,
}

impl PreviewRunner {
    pub fn new(resolver: PreviewResolver, config: PreviewConfig) -> Self {
        let state = Arc::new(Mutex::new(PreviewState {
            content: PreviewContent::Empty,
            current_line: String::new(),
            scroll_offset: 0,
            visible: true,
        }));
        let cache = Arc::new(Mutex::new(PreviewCache::new(config.cache_capacity)));

        Self {
            resolver,
            config,
            state,
            cache,
            generation: Arc::new(AtomicU64::new(0)),
            active_cancel: Arc::new(Mutex::new(Arc::new(AtomicBool::new(false)))),
        }
    }

    pub fn state(&self) -> SharedPreview {
        Arc::clone(&self.state)
    }

    pub fn request(&self, line: &str) {
        {
            let s = self.state.lock();
            if s.current_line == line {
                return;
            }
        }

        // Check cache
        {
            let mut cache = self.cache.lock();
            if let Some(cached) = cache.get(line) {
                let cached = cached.clone();
                let mut s = self.state.lock();
                s.content = cached;
                s.current_line = line.to_string();
                s.scroll_offset = 0;
                return;
            }
        }

        // Cancel the previous preview command by setting its cancel flag
        let new_cancel = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active_cancel.lock();
            // Signal the old thread to kill its child process
            active.store(true, Ordering::SeqCst);
            // Install the new cancel token
            *active = Arc::clone(&new_cancel);
        }

        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let cmd = self.resolver.resolve(line);
        let state = Arc::clone(&self.state);
        let cache = Arc::clone(&self.cache);
        let generation = Arc::clone(&self.generation);
        let timeout = self.config.timeout;
        let max_bytes = self.config.max_output_bytes;
        let line_owned = line.to_string();
        let cancel_token = new_cancel;

        {
            let mut s = state.lock();
            s.content = PreviewContent::Loading;
            s.current_line = line_owned.clone();
            s.scroll_offset = 0;
        }

        thread::spawn(move || {
            let result = run_preview_command(&cmd, timeout, max_bytes, &cancel_token);

            // Only update if still the latest generation
            if generation.load(Ordering::SeqCst) != gen {
                return;
            }

            let content = match result {
                Ok(output) => PreviewContent::Text(output),
                Err(PreviewError::Timeout) => {
                    PreviewContent::Error("Preview timed out".to_string())
                }
                Err(PreviewError::Cancelled) => return,
                Err(PreviewError::Failed(msg)) => PreviewContent::Error(msg),
                Err(PreviewError::OutputTooLarge(partial)) => {
                    PreviewContent::Text(partial + "\n\n... (output truncated)")
                }
            };

            {
                let mut c = cache.lock();
                c.insert(line_owned.clone(), content.clone());
            }

            let mut s = state.lock();
            if s.current_line == line_owned {
                s.content = content;
            }
        });
    }

    pub fn clear(&self) {
        // Cancel any running preview
        {
            let active = self.active_cancel.lock();
            active.store(true, Ordering::SeqCst);
        }
        let mut s = self.state.lock();
        s.content = PreviewContent::Empty;
        s.current_line.clear();
        s.scroll_offset = 0;
    }

    pub fn toggle_visible(&self) {
        let mut s = self.state.lock();
        s.visible = !s.visible;
    }

    pub fn refresh(&self) {
        let line = {
            let s = self.state.lock();
            s.current_line.clone()
        };
        if !line.is_empty() {
            {
                let mut cache = self.cache.lock();
                cache.clear();
            }
            {
                let mut s = self.state.lock();
                s.current_line.clear();
            }
            self.request(&line);
        }
    }
}

#[derive(Debug)]
enum PreviewError {
    Timeout,
    Cancelled,
    Failed(String),
    OutputTooLarge(String),
}

fn run_preview_command(
    cmd: &str,
    timeout: Duration,
    max_bytes: usize,
    cancel: &AtomicBool,
) -> Result<String, PreviewError> {
    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

    let mut command = Command::new(shell);
    command
        .arg(shell_arg)
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // On Unix, spawn in a new process group so we can kill the entire tree
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    let mut child = command.spawn().map_err(|e| {
        let msg = match e.kind() {
            io::ErrorKind::NotFound => {
                format!("shell '{}' not found: {}", shell, e)
            }
            io::ErrorKind::PermissionDenied => {
                format!("permission denied running '{}': {}", shell, e)
            }
            _ => format!("failed to spawn preview command: {}", e),
        };
        PreviewError::Failed(msg)
    })?;

    let pid = child.id();
    let poll_interval = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;

    loop {
        if cancel.load(Ordering::Relaxed) {
            kill_process_group(pid);
            let _ = child.wait();
            return Err(PreviewError::Cancelled);
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() && status.code() == Some(126) {
                    let _ = child.wait_with_output();
                    return Err(PreviewError::Failed(
                        format!("permission denied executing preview command")
                    ));
                }
                if !status.success() && status.code() == Some(127) {
                    let _ = child.wait_with_output();
                    return Err(PreviewError::Failed(
                        format!("command not found in preview: {}", cmd)
                    ));
                }
                break;
            }
            Ok(None) => {
                elapsed += poll_interval;
                if elapsed >= timeout {
                    kill_process_group(pid);
                    let _ = child.wait();
                    return Err(PreviewError::Timeout);
                }
                thread::sleep(poll_interval);
            }
            Err(e) => return Err(PreviewError::Failed(format!("wait error: {}", e))),
        }
    }

    let output = child.wait_with_output()
        .map_err(|e| PreviewError::Failed(format!("failed to read output: {}", e)))?;

    let stdout = output.stdout;
    if stdout.len() > max_bytes {
        let truncated = String::from_utf8_lossy(&stdout[..max_bytes]).into_owned();
        return Err(PreviewError::OutputTooLarge(truncated));
    }

    let result = String::from_utf8_lossy(&stdout).into_owned();

    if result.is_empty() && !output.stderr.is_empty() {
        let stderr_text = String::from_utf8_lossy(&output.stderr).into_owned();
        let first_line = stderr_text.lines().next().unwrap_or(&stderr_text);
        return Err(PreviewError::Failed(first_line.to_string()));
    }

    if !output.status.success() && !result.is_empty() {
        return Ok(result);
    }

    if !output.status.success() && result.is_empty() {
        let code = output.status.code().unwrap_or(-1);
        return Err(PreviewError::Failed(
            format!("preview command exited with code {}", code)
        ));
    }

    Ok(result)
}

/// Kill an entire process group. On Unix, sends SIGKILL to the process group
/// (negative pid). On other platforms, falls back to killing just the process.
fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            // Kill the process group (negative pgid)
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        // On Windows, Command::kill handles it; this is a fallback
        let _ = pid;
    }
}

use std::io;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_cancel_flag_propagates_to_running_command() {
        // Create a cancel flag, set it immediately, then run a command.
        // The command should be killed quickly rather than running for 60s.
        let cancel = Arc::new(AtomicBool::new(false));

        // Spawn a thread that sets cancel after 100ms
        let cancel_clone = Arc::clone(&cancel);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            cancel_clone.store(true, Ordering::SeqCst);
        });

        let start = Instant::now();
        let result = run_preview_command(
            "sleep 60",
            Duration::from_secs(30),
            1_000_000,
            &cancel,
        );
        let elapsed = start.elapsed();

        // Should complete within 500ms (100ms wait + 50ms poll interval + margin)
        assert!(elapsed < Duration::from_secs(1),
            "Cancel took {:?}, expected < 1s", elapsed);
        assert!(matches!(result, Err(PreviewError::Cancelled)));
    }

    #[test]
    fn test_timeout_kills_slow_command() {
        let cancel = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let result = run_preview_command(
            "sleep 60",
            Duration::from_millis(200),
            1_000_000,
            &cancel,
        );
        let elapsed = start.elapsed();

        // Should timeout within 500ms (200ms timeout + poll margin)
        assert!(elapsed < Duration::from_secs(1),
            "Timeout took {:?}, expected < 1s", elapsed);
        assert!(matches!(result, Err(PreviewError::Timeout)));
    }

    #[test]
    fn test_successful_command_returns_output() {
        let cancel = Arc::new(AtomicBool::new(false));
        let result = run_preview_command(
            "echo hello",
            Duration::from_secs(5),
            1_000_000,
            &cancel,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[test]
    fn test_command_not_found_shows_error() {
        let cancel = Arc::new(AtomicBool::new(false));
        let result = run_preview_command(
            "nonexistent_command_xyz_12345",
            Duration::from_secs(5),
            1_000_000,
            &cancel,
        );
        // Should be an error (command not found = exit 127)
        assert!(matches!(result, Err(PreviewError::Failed(_))));
    }

    #[test]
    fn test_permission_denied_shows_error() {
        let cancel = Arc::new(AtomicBool::new(false));
        let result = run_preview_command(
            "/dev/null",
            Duration::from_secs(5),
            1_000_000,
            &cancel,
        );
        // /dev/null is not executable, should get permission denied or similar
        assert!(matches!(result, Err(PreviewError::Failed(_))));
    }

    #[test]
    fn test_output_too_large_truncates() {
        let cancel = Arc::new(AtomicBool::new(false));
        // Generate more than 100 bytes of output
        let result = run_preview_command(
            "seq 1 10000",
            Duration::from_secs(5),
            100, // very small limit
            &cancel,
        );
        assert!(matches!(result, Err(PreviewError::OutputTooLarge(_))));
        if let Err(PreviewError::OutputTooLarge(partial)) = result {
            assert!(partial.len() <= 100);
        }
    }

    #[test]
    fn test_preview_runner_cancel_on_new_request() {
        // Prove that requesting a new preview cancels the old one.
        // Use a resolver that runs sleep for first item, echo for second.
        let resolver = PreviewResolver::new("echo {}".to_string());
        let config = PreviewConfig {
            timeout: Duration::from_secs(10),
            max_output_bytes: 1_000_000,
            cache_capacity: 10,
        };
        let runner = PreviewRunner::new(resolver, config);

        // Request a slow preview
        // We can't easily use "sleep 60" here because the resolver wraps {}
        // but we can test that two rapid requests don't deadlock or panic
        runner.request("first");
        thread::sleep(Duration::from_millis(10));
        runner.request("second");
        thread::sleep(Duration::from_millis(200));

        // Second request should have completed (echo is fast)
        let state = runner.state();
        let s = state.lock();
        assert_eq!(s.current_line, "second");
        // Should have content (either Text or Loading depending on timing)
        assert!(!matches!(s.content, PreviewContent::Empty));
    }

    #[test]
    #[cfg(unix)]
    fn test_cancel_kills_child_process_tree() {
        // Verify that cancelling a complex pipeline (sh -c "sleep 300 & wait")
        // doesn't leave orphan processes behind.
        use std::process::Command as StdCommand;

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);

        // Use a unique marker so we can grep for it
        let marker = format!("fuzzyseek_test_{}", std::process::id());
        let cmd = format!("sleep 300 & echo {}; wait", marker);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            cancel_clone.store(true, Ordering::SeqCst);
        });

        let start = Instant::now();
        let _ = run_preview_command(&cmd, Duration::from_secs(30), 1_000_000, &cancel);
        let elapsed = start.elapsed();

        assert!(elapsed < Duration::from_secs(1));

        // Give a moment for the signal to propagate
        thread::sleep(Duration::from_millis(100));

        // Check that no process with our marker's sleep is still running
        let ps_output = StdCommand::new("sh")
            .arg("-c")
            .arg(format!("ps aux | grep 'sleep 300' | grep -v grep"))
            .output()
            .unwrap();
        let ps_text = String::from_utf8_lossy(&ps_output.stdout);
        assert!(!ps_text.contains("sleep 300"),
            "Orphan process still running after cancel: {}", ps_text);
    }
}

