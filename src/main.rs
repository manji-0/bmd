//! bmd — a TUI markdown viewer with vim bindings, rich markup, native mermaid
//! rendering, responsive tables, and macOS browser link opening.

use std::{
    env, fs,
    io::{self, Read},
    path::PathBuf,
    process,
    time::Duration,
};

use crossterm::{
    ExecutableCommand,
    event::{DisableMouseCapture, EnableMouseCapture},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use ratatui_image::picker::{Picker, cap_parser::QueryStdioOptions};

use bmd::app::App;
use bmd::error::AppError;
use bmd::parse::parse_with_path;

fn main() {
    if let Err(e) = run() {
        eprintln!("bmd: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let (input, base_path) = read_input()?;
    let document = parse_with_path(base_path.as_deref(), &input)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;

    // Query the terminal (Ghostty, Kitty, iTerm2, etc.) for native graphics support.
    // Use a short timeout so an immediate 'q' is not delayed if the terminal does
    // not respond quickly.
    let options = QueryStdioOptions {
        timeout: Duration::from_millis(200),
        ..QueryStdioOptions::default()
    };
    let picker =
        Picker::from_query_stdio_with_options(options).unwrap_or_else(|_| Picker::halfblocks());

    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| AppError::TerminalSetup(e.to_string()))?;

    let source_label = base_path.as_ref().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    });
    let app = App::new(document, picker, base_path, source_label)?;
    let result = app.run(&mut terminal);

    restore_terminal(&mut terminal)?;
    result
}

fn read_input() -> Result<(String, Option<PathBuf>), AppError> {
    match env::args().nth(1) {
        Some(path) if path != "-" => {
            let path = PathBuf::from(path);
            let content = fs::read_to_string(&path).map_err(AppError::Io)?;
            Ok((content, Some(path)))
        }
        _ => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            Ok((buffer, None))
        }
    }
}

fn restore_terminal<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), AppError> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(DisableMouseCapture)?;
    stdout.execute(LeaveAlternateScreen)?;
    terminal
        .show_cursor()
        .map_err(|e| AppError::TerminalSetup(e.to_string()))?;
    Ok(())
}
