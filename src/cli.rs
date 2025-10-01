use clap::{Args, Parser, Subcommand};

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

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    Init,
    Render(RenderArgs),
}

#[derive(Args, Clone, Debug)]
pub struct RenderArgs {
    #[arg(long)]
    pub posts: bool,
    #[arg(long = "static")]
    pub static_assets: bool,
    #[arg(long, conflicts_with = "force")]
    pub changed: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(short, long)]
    pub verbose: bool,
}
