mod app;
mod config;
mod input;
mod keybind;
mod matcher;
mod preview;
mod theme;
mod ui;

use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use clap::Parser;
use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};

use app::App;
use config::Config;

const SHELL_BASH: &str = include_str!("../shell/fuzzyseek.bash");
const SHELL_ZSH: &str = include_str!("../shell/fuzzyseek.zsh");
const SHELL_FISH: &str = include_str!("../shell/fuzzyseek.fish");

#[derive(Parser, Debug)]
#[command(name = "fuzzyseek", version, about = "High-performance fuzzy finder for millions of lines")]
pub struct Cli {
    /// Input file (reads from stdin if not provided)
    #[arg(short, long)]
    file: Option<String>,

    /// Command to execute for input (alternative to pipe/file)
    #[arg(long)]
    cmd: Option<String>,

    /// Enable multi-select mode (Tab/Ctrl+Space to toggle)
    #[arg(short, long)]
    multi: bool,

    /// Preview command (use {} as placeholder for the line)
    #[arg(short, long)]
    preview: Option<String>,

    /// Preview timeout in milliseconds
    #[arg(long, default_value = "5000")]
    preview_timeout: u64,

    /// Initial query
    #[arg(short, long, default_value = "")]
    query: String,

    /// Height in lines (0 = fullscreen)
    #[arg(long, default_value = "0")]
    height: u16,

    /// Custom delimiter for output (default: newline)
    #[arg(short, long, default_value = "\n")]
    delimiter: String,

    /// Key bindings (format: action:key, e.g. "confirm:ctrl-m,cancel:ctrl-g")
    #[arg(long)]
    bind: Vec<String>,

    /// Theme name ("dark" or "light")
    #[arg(long)]
    theme: Option<String>,

    /// Maximum number of items to read (OOM protection)
    #[arg(long)]
    max_items: Option<usize>,

    /// Print shell integration script and exit (bash, zsh, fish)
    #[arg(long, value_name = "SHELL")]
    shell_integration: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Handle --shell-integration: print script and exit
    if let Some(ref shell_name) = cli.shell_integration {
        let script = match shell_name.to_lowercase().as_str() {
            "bash" => SHELL_BASH,
            "zsh" => SHELL_ZSH,
            "fish" => SHELL_FISH,
            other => {
                eprintln!("fuzzyseek: unsupported shell '{}' (use bash, zsh, or fish)", other);
                return ExitCode::from(2);
            }
        };
        print!("{}", script);
        return ExitCode::SUCCESS;
    }

    let (config, conflicts) = match Config::from_cli(&cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("fuzzyseek: {}", e);
            return ExitCode::from(2);
        }
    };

    // Report bind conflicts as warnings
    for conflict in &conflicts {
        eprintln!(
            "fuzzyseek: warning: key '{}' reassigned from {:?} to {:?}",
            conflict.key.display(),
            conflict.displaced_action,
            conflict.new_action
        );
    }

    let input_source = match Config::determine_input_source(&cli) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("fuzzyseek: {}", e);
            return ExitCode::from(2);
        }
    };

    let is_inline = config.height > 0;
    let inline_height = config.height;

    let mut stderr_handle = io::stderr();
    if !stderr_handle.is_terminal() {
        eprintln!("fuzzyseek: stderr is not a terminal, cannot display TUI");
        return ExitCode::from(2);
    }

    if let Err(e) = enable_raw_mode() {
        eprintln!("fuzzyseek: failed to enable raw mode: {}", e);
        return ExitCode::from(2);
    }

    if !is_inline {
        if let Err(e) = execute!(stderr_handle, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            eprintln!("fuzzyseek: failed to initialize terminal: {}", e);
            return ExitCode::from(2);
        }
    } else {
        let _ = execute!(stderr_handle, EnableMouseCapture);
    }

    let backend = CrosstermBackend::new(io::stderr());
    let terminal = if is_inline {
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(inline_height),
            },
        )
    } else {
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Fullscreen,
            },
        )
    };

    let mut terminal = match terminal {
        Ok(t) => t,
        Err(e) => {
            let _ = disable_raw_mode();
            if !is_inline {
                let _ = execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
            }
            eprintln!("fuzzyseek: failed to create terminal: {}", e);
            return ExitCode::from(2);
        }
    };

    let delimiter = cli.delimiter.clone();
    let mut app = App::new(config, input_source);
    let result = app.run(&mut terminal);

    let _ = disable_raw_mode();
    if is_inline {
        let _ = execute!(io::stderr(), DisableMouseCapture, cursor::Show);
        let _ = execute!(io::stderr(), cursor::MoveToNextLine(1));
    } else {
        let _ = execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
    }

    match result {
        Ok(Some(selections)) => {
            let output = selections.join(&delimiter);
            print!("{}", output);
            io::stdout().flush().ok();
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::from(130),
        Err(e) => {
            eprintln!("fuzzyseek: error: {}", e);
            ExitCode::from(2)
        }
    }
}
