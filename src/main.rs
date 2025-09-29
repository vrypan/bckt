mod cli;
mod commands;
pub mod config;
pub mod content;
pub mod markdown;
pub mod render;
pub mod template;

fn main() {
    let app = cli::Cli::build();
    let outcome = commands::run(app.command);

    if let Err(problem) = outcome {
        eprintln!("{problem}");
        std::process::exit(1);
    }
}
