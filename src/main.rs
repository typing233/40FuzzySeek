mod app;
mod config;
mod input;
mod keybind;
mod matcher;
mod preview;
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

#[derive(Parser, Debug)]
#[command(name = "fuzzyseek", version, about = "High-performance fuzzy finder for millions of lines")]
struct Cli {
    /// Input file (reads from stdin if not provided)
    #[arg(short, long)]
    file: Option<String>,

    /// Enable multi-select mode (Tab to toggle)
    #[arg(short, long)]
    multi: bool,

    /// Preview command (use {} as placeholder for the line)
    #[arg(short, long)]
    preview: Option<String>,

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let config = match Config::from_cli(&cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("fuzzyseek: {}", e);
            return ExitCode::from(2);
        }
    };

    let input_source = if let Some(ref path) = cli.file {
        if !std::path::Path::new(path).exists() {
            eprintln!("fuzzyseek: cannot open '{}': No such file or directory", path);
            return ExitCode::from(2);
        }
        input::InputSource::File(path.clone())
    } else if !io::stdin().is_terminal() {
        input::InputSource::Stdin
    } else {
        eprintln!("fuzzyseek: no input (provide --file or pipe data via stdin)");
        return ExitCode::from(2);
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

    let mut app = App::new(config, input_source);
    let result = app.run(&mut terminal);

    let _ = disable_raw_mode();
    if is_inline {
        let _ = execute!(io::stderr(), DisableMouseCapture, cursor::Show);
        // Insert newline to move below the inline viewport
        let _ = execute!(io::stderr(), cursor::MoveToNextLine(1));
    } else {
        let _ = execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
    }

    match result {
        Ok(Some(selections)) => {
            let output = selections.join(&cli.delimiter);
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
