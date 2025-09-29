use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bucket3")]
#[command(about = "Static site generator toolkit", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn build() -> Self {
        <Self as Parser>::parse()
    }
}

#[derive(Subcommand, Clone, Copy, Debug)]
pub enum Command {
    Init,
}
