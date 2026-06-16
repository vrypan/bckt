use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bckt", version)]
#[command(
    about = "Build and preview statically generated blogs",
    long_about = "bckt is an opinionated but flexible static site generator for blogs. \n\
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
        long_about = "Initialise a new bckt workspace in the current directory.\n\
The command is idempotent: existing files are left untouched, so you can rerun it\n\
to ensure required folders and templates are present without overwriting customisations."
    )]
    Init(InitArgs),
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
        long_about = "Delete the previously rendered html/ directory and the incremental cache (stored in .bckt).\n\
The command recreates html/ so the next render starts from a clean slate.",
        alias = "clear"
    )]
    Clean(CleanArgs),
    #[command(
        about = "Inspect and switch between installed themes",
        long_about = "List the themes stored in themes/ or apply a different one to the current project.\n\
Applying a theme copies its templates and assets into place and updates bckt.yaml."
    )]
    Themes(ThemesArgs),
    #[command(
        about = "Query configuration values from bckt.yaml",
        long_about = "Read configuration values from bckt.yaml or get the project root path.\n\
Use this command from any subdirectory within the project to retrieve config values."
    )]
    Config(ConfigArgs),
}

#[derive(Args, Clone, Debug)]
pub struct InitArgs {
    #[arg(
        long,
        help = "Project root directory (defaults to current directory)",
        long_help = "Specify the project root directory. Supports tilde expansion (e.g., ~/myblog). If not provided, uses the current working directory."
    )]
    pub root: Option<String>,
    #[arg(
        long,
        help = "Theme to initialise the project with (a .zip/directory path or a theme name)",
        long_help = "Theme to install into the new project. Provide a path to a .zip archive or a theme directory, or a bare theme name resolved across the theme search path (BCKT_THEME_PATH and the directory containing the bckt executable), preferring <name>.zip and falling back to a <name>/ directory. Defaults to 'bckt3'."
    )]
    pub theme: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct RenderArgs {
    #[arg(
        long,
        help = "Project root directory (defaults to current directory)",
        long_help = "Specify the project root directory. Supports tilde expansion (e.g., ~/myblog). If not provided, uses the current working directory."
    )]
    pub root: Option<String>,
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
        help = "Project root directory (defaults to current directory)",
        long_help = "Specify the project root directory. Supports tilde expansion (e.g., ~/myblog). If not provided, uses the current working directory."
    )]
    pub root: Option<String>,
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
        help = "Rebuild everything on each change instead of using the incremental cache",
        long_help = "Run full rebuilds while the dev server watches for edits. By default the dev server performs incremental renders."
    )]
    pub force: bool,
    #[arg(
        long,
        help = "Show verbose logs from the watcher and render pipeline",
        long_help = "Display the same detailed progress output as `render --verbose` while the dev server is running."
    )]
    pub verbose: bool,
}

#[derive(Args, Clone, Debug)]
pub struct CleanArgs {
    #[arg(
        long,
        help = "Project root directory (defaults to current directory)",
        long_help = "Specify the project root directory. Supports tilde expansion (e.g., ~/myblog). If not provided, uses the current working directory."
    )]
    pub root: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct ThemesArgs {
    #[arg(
        long,
        help = "Project root directory (defaults to current directory)",
        long_help = "Specify the project root directory. Supports tilde expansion (e.g., ~/myblog). If not provided, uses the current working directory."
    )]
    pub root: Option<String>,
    #[command(subcommand)]
    pub command: ThemesSubcommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum ThemesSubcommand {
    #[command(
        about = "List themes installed under themes/",
        long_about = "Show the themes available in the local themes/ directory and mark which one is currently active."
    )]
    List,
    #[command(
        about = "Apply a theme by copying its templates and static files",
        long_about = "Copy templates/ and skel/ from the selected theme into the project directories and update bckt.yaml to reference it. Existing pages/ are left untouched."
    )]
    Use {
        #[arg(help = "Name of the theme directory inside themes/")]
        name: String,
        #[arg(
            long,
            help = "Overwrite templates/ and skel/ without prompting",
            long_help = "Bypass the confirmation prompt when the destination directories already contain files."
        )]
        force: bool,
    },
    #[command(
        about = "Install a theme from a local .zip archive or directory into themes/",
        long_about = "Install a local theme into themes/<name> by extracting a .zip archive or copying a theme directory. The source should contain the theme directories (templates/, skel/, pages/) at its root."
    )]
    Install(ThemeInstallArgs),
}

#[derive(Args, Clone, Debug)]
pub struct ThemeInstallArgs {
    #[arg(help = "Path to a .zip archive or a theme directory")]
    pub path: String,
    #[arg(
        long,
        help = "Directory name created under themes/ (defaults to the archive file name)"
    )]
    pub name: Option<String>,
    #[arg(long, help = "Overwrite an existing theme directory")]
    pub force: bool,
}

#[derive(Args, Clone, Debug)]
pub struct ConfigArgs {
    #[arg(
        long,
        help = "Project root directory (defaults to current directory)",
        long_help = "Specify the project root directory. Supports tilde expansion (e.g., ~/myblog). If not provided, uses the current working directory."
    )]
    pub root: Option<String>,
    #[arg(long = "root-dir", help = "Get the project root directory path")]
    pub root_dir: bool,
    #[arg(long, help = "Get the base_url configuration value")]
    pub base_url: bool,
    #[arg(long, help = "Get the title configuration value")]
    pub title: bool,
    #[arg(long, help = "Get the homepage_posts configuration value")]
    pub homepage_posts: bool,
    #[arg(long, help = "Get the date_format configuration value")]
    pub date_format: bool,
    #[arg(long, help = "Get the default_timezone configuration value")]
    pub default_timezone: bool,
    #[arg(long, help = "Get the theme configuration value")]
    pub theme: bool,
    #[arg(long, help = "Get the search.asset_path configuration value")]
    pub search_asset_path: bool,
    #[arg(long, help = "Get the search.default_language configuration value")]
    pub search_default_language: bool,
}
