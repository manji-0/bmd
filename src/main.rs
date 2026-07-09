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
use bmd::github::{self, GitHubAuth, GitHubUrl};
use bmd::parse::{MarkupFormat, parse_document, parse_with_path};

fn main() {
    if let Err(e) = run() {
        eprintln!("bmd: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let (document, base_path, source_label, github_auth) = read_input()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;

    let options = QueryStdioOptions {
        timeout: Duration::from_millis(200),
        ..QueryStdioOptions::default()
    };
    let picker =
        Picker::from_query_stdio_with_options(options).unwrap_or_else(|_| Picker::halfblocks());

    if env::var("BMD_DEBUG_PICKER").is_ok() {
        eprintln!(
            "[bmd] picker protocol={:?} font_size={:?}",
            picker.protocol_type(),
            picker.font_size()
        );
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| AppError::TerminalSetup(e.to_string()))?;

    let mut app = App::new(document, picker, base_path, source_label)?;
    app.set_github_auth(github_auth);
    let result = app.run(&mut terminal);

    restore_terminal(&mut terminal)?;
    result
}

type ReadInputResult = (
    bmd::domain::Document,
    Option<PathBuf>,
    Option<String>,
    Option<GitHubAuth>,
);

fn read_input() -> Result<ReadInputResult, AppError> {
    match env::args().nth(1) {
        Some(arg) if arg != "-" => {
            if let Some(github_url) = github::parse_github_url(&arg) {
                let auth = github::resolve_auth();
                match github_url {
                    GitHubUrl::Blob(blob) => {
                        eprintln!("fetching {}...", blob.path);
                        let content = github::fetch_blob_content(&blob, &auth)
                            .map_err(|e| AppError::GitHubFetch(e.to_string()))?;
                        let format = MarkupFormat::from_path(std::path::Path::new(&blob.path))
                            .unwrap_or(MarkupFormat::Markdown);
                        let mut document = parse_document(format, &content)?;
                        github::rewrite_relative_links(&mut document, &blob);
                        let source_label = Some(blob.path.clone());
                        Ok((document, None, source_label, Some(auth)))
                    }
                    GitHubUrl::PullRequest(pr) => {
                        eprintln!("fetching PR #{}...", pr.number);
                        let info = github::fetch_pr_info(&pr, &auth)
                            .map_err(|e| AppError::GitHubFetch(e.to_string()))?;
                        let source_label =
                            Some(format!("PR #{}: {}", pr.number, info.title));
                        let markdown = github::build_pr_listing_markdown(&pr, &info);
                        let document = parse_document(MarkupFormat::Markdown, &markdown)?;
                        Ok((document, None, source_label, Some(auth)))
                    }
                }
            } else {
                let path = PathBuf::from(&arg);
                let content = fs::read_to_string(&path).map_err(AppError::Io)?;
                let document = parse_with_path(Some(&path), &content)?;
                let source_label = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned());
                Ok((document, Some(path), source_label, None))
            }
        }
        _ => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            let document = parse_with_path(None, &buffer)?;
            Ok((document, None, None, None))
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
