mod app;
mod config;
mod input;
mod matcher;
mod ui;

use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let config = Config::from_cli(&cli);

    let input_source = if let Some(ref path) = cli.file {
        input::InputSource::File(path.clone())
    } else if !io::stdin().is_terminal() {
        input::InputSource::Stdin
    } else {
        eprintln!("fuzzyseek: no input (provide --file or pipe data via stdin)");
        return ExitCode::from(2);
    };

    let stderr = io::stderr();
    let backend = CrosstermBackend::new(stderr.lock());

    enable_raw_mode().expect("failed to enable raw mode");
    let mut stderr_handle = io::stderr();
    execute!(stderr_handle, EnterAlternateScreen, EnableMouseCapture)
        .expect("failed to enter alternate screen");

    let mut terminal = Terminal::new(backend).expect("failed to create terminal");

    let mut app = App::new(config, input_source);
    let result = app.run(&mut terminal);

    disable_raw_mode().expect("failed to disable raw mode");
    execute!(
        io::stderr(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .expect("failed to leave alternate screen");

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
