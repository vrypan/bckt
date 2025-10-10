mod clean;
mod config;
mod dev;
mod init;
mod render;
mod themes;

use anyhow::Result;

use crate::cli::Command;

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Init(args) => init::run_init_command(args),
        Command::Render(args) => render::run_render_command(args),
        Command::Dev(args) => dev::run_dev_command(args),
        Command::Clean(args) => clean::run_clean_command(args),
        Command::Themes(args) => themes::run_themes_command(args),
        Command::Config(args) => config::run_config_command(args),
    }
}
