mod dev;
mod init;
mod render;

use anyhow::Result;

use crate::cli::Command;

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::Init => init::run_init_command(),
        Command::Render(args) => render::run_render_command(args),
        Command::Dev(args) => dev::run_dev_command(args),
    }
}
