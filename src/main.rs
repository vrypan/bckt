mod cli;
mod commands;

use crate::cli::Command;

fn main() {
    let app = cli::Cli::build();
    let outcome = commands::run(app.command);

    if let Err(problem) = outcome {
        eprintln!("{problem}");
        std::process::exit(1);
    }
}
