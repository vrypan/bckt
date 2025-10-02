use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bucket3", version)]
#[command(
    about = "Build and preview microblog-friendly static sites",
    long_about = "Bucket3rs is a static-site toolbox tailored for microblogging. \n\
Use the bundled commands to scaffold a workspace, render posts, preview locally, \n\
or clean out generated artifacts before a fresh build."
)]
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
    #[command(
        about = "Create the starter directories, templates, and config",
        long_about = "Initialise a new bucket3rs workspace in the current directory.\n\
The command is idempotent: existing files are left untouched, so you can rerun it\n\
to ensure required folders and templates are present without overwriting customisations."
    )]
    Init,
    #[command(
        about = "Render posts and assets into the html/ output tree",
        long_about = "Transform your source posts and static assets into publish-ready HTML.\n\
By default both posts and static assets are processed. Combine the flags below to\n\
limit the run or switch between full and incremental rebuilds."
    )]
    Render(RenderArgs),
    #[command(
        about = "Run the file-watching development server",
        long_about = "Serve the generated html/ directory over HTTP and watch your sources for changes.\n\
The development server recompiles on edits so you can preview posts live while iterating."
    )]
    Dev(DevArgs),
    #[command(
        about = "Remove generated files from html/",
        long_about = "Delete the previously rendered html/ directory and the incremental cache (stored in .bucket3).\n\
The command recreates html/ so the next render starts from a clean slate.",
        alias = "clear"
    )]
    Clean,
}

#[derive(Args, Clone, Debug)]
pub struct RenderArgs {
    #[arg(
        long,
        help = "Render post content and attachments only (skip copying static assets)",
        long_help = "Render only the posts pipeline. When supplied on its own the static asset step is skipped so you can focus on Markdown/HTML sources."
    )]
    pub posts: bool,
    #[arg(
        long = "static",
        help = "Copy files from skel/ without rendering posts",
        long_help = "Limit the run to static assets housed in skel/. Combine with --force if you want to refresh the static tree regardless of change detection."
    )]
    pub static_assets: bool,
    #[arg(
        long,
        conflicts_with = "force",
        help = "Reuse the incremental cache to rebuild only changed inputs",
        long_help = "Skips work for posts whose content and assets are unchanged. Template or config changes still trigger the necessary downstream rebuilds."
    )]
    pub changed: bool,
    #[arg(
        long,
        help = "Ignore caches and rebuild everything from scratch",
        long_help = "Disables incremental shortcuts and regenerates every post, feed, and asset. Use this after large refactors or when you suspect the cache is stale."
    )]
    pub force: bool,
    #[arg(
        short,
        long,
        help = "Print progress information while rendering",
        long_help = "Show which posts are rendered or skipped, along with timing breakdowns for each pipeline stage."
    )]
    pub verbose: bool,
}

#[derive(Args, Clone, Debug)]
pub struct DevArgs {
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Interface to bind the development server to",
        long_help = "Set an alternate host/IP address for the dev server. Defaults to 127.0.0.1 so it only listens locally."
    )]
    pub host: String,
    #[arg(
        long,
        default_value_t = 4000,
        help = "Port number for the development server",
        long_help = "Pick a custom TCP port for the dev server. 4000 is the default."
    )]
    pub port: u16,
    #[arg(
        long,
        help = "Enable incremental rebuilds while watching files",
        long_help = "Reuse the same incremental cache that `render --changed` uses so edits touch the minimum number of posts during dev."
    )]
    pub changed: bool,
    #[arg(
        long,
        help = "Show verbose logs from the watcher and render pipeline",
        long_help = "Display the same detailed progress output as `render --verbose` while the dev server is running."
    )]
    pub verbose: bool,
}
