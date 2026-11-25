use anyhow::Result;
use clap::Parser;
use crossterm::execute;
use std::io::IsTerminal;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::stdout;
use std::path::PathBuf;

mod buffer;
mod clipboard;
mod commands;
mod config;
mod editor;
mod keytree;
mod plugins;
mod search;
mod syntax;
mod ui;
mod undo;

#[derive(Parser, Debug)]
#[command(name = "micro-rs", version, about = "A modern terminal text editor in Rust")]
struct Args {
    /// File(s) to open
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.files.is_empty() && !std::io::stdin().is_terminal() {
        use std::io::Read;
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        let path = std::env::current_dir()?.join(".micro-rs-stdin.tmp");
        std::fs::write(&path, s)?;
        return run(vec![path]);
    }
    run(args.files)
}

fn run(files: Vec<PathBuf>) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut editor = editor::Editor::new(files);
    let result = (|| -> Result<()> {
        while !editor.quit {
            terminal.draw(|f| ui::draw(f, &editor))?;
            editor.run_step()?;
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), crossterm::event::DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}
