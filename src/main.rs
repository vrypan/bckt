mod cli;
mod commands;
pub mod config;
pub mod content;
pub mod language;
pub mod markdown;
pub mod render;
pub mod search;
pub mod template;
pub mod theme;
pub mod utils;

fn main() {
    let app = cli::Cli::build();
    let outcome = commands::run(app.command);

    if let Err(problem) = outcome {
        // Alternate Display ("{:#}") prints the whole anyhow cause chain, so a
        // wrapped error (e.g. "initial render ... failed") still surfaces its
        // underlying cause instead of just the outermost context.
        eprintln!("Error: {problem:#}");
        std::process::exit(1);
    }
}
