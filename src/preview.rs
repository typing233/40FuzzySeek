use std::process::Command;
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;

pub struct PreviewState {
    pub content: String,
    pub loading: bool,
    current_line: String,
}

pub type SharedPreview = Arc<Mutex<PreviewState>>;

pub struct PreviewRunner {
    cmd_template: String,
    state: SharedPreview,
    generation: Arc<std::sync::atomic::AtomicU64>,
}

impl PreviewRunner {
    pub fn new(cmd_template: String) -> Self {
        let state = Arc::new(Mutex::new(PreviewState {
            content: String::new(),
            loading: false,
            current_line: String::new(),
        }));
        Self {
            cmd_template,
            state,
            generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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

        let gen = self.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let cmd = self.cmd_template.replace("{}", line);
        let state = Arc::clone(&self.state);
        let generation = Arc::clone(&self.generation);
        let line_owned = line.to_string();

        {
            let mut s = state.lock();
            s.loading = true;
            s.current_line = line_owned.clone();
        }

        thread::spawn(move || {
            let output = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output();

            // Only update if we're still the latest request
            if generation.load(std::sync::atomic::Ordering::SeqCst) != gen {
                return;
            }

            let mut s = state.lock();
            if s.current_line != line_owned {
                return;
            }
            s.loading = false;
            s.content = match output {
                Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
                Err(e) => format!("Preview error: {}", e),
            };
        });
    }

    pub fn clear(&self) {
        let mut s = self.state.lock();
        s.content.clear();
        s.loading = false;
        s.current_line.clear();
    }
}
