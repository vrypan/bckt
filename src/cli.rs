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
        help = "URL of a zip archive containing the theme to initialise the project with",
        long_help = "Provide an HTTP(S) URL pointing to a zip archive. The archive is downloaded, extracted, and stored under themes/<theme-name>."
    )]
    pub theme_url: Option<String>,
    #[arg(
        long,
        help = "GitHub repository in the form owner/repo to fetch the theme from",
        long_help = "Fetch the initial theme from a GitHub repository instead of a direct zip. Combine with --theme-branch or --theme-tag and --theme-subdir to pinpoint the desired folder."
    )]
    pub theme_github: Option<String>,
    #[arg(
        long,
        requires = "theme_github",
        conflicts_with = "theme_branch",
        help = "Git tag to download when using --theme-github",
        long_help = "Select a specific Git tag when downloading from GitHub. If omitted, bckt falls back to a tag that matches the binary version and then the main branch."
    )]
    pub theme_tag: Option<String>,
    #[arg(
        long,
        requires = "theme_github",
        conflicts_with = "theme_tag",
        help = "Git branch to download when using --theme-github",
        long_help = "Select a branch when downloading from GitHub."
    )]
    pub theme_branch: Option<String>,
    #[arg(
        long,
        help = "Path inside the archive or repository that contains the theme",
        long_help = "Specify the subdirectory within the downloaded archive that represents the theme (for example themes/bckt3)."
    )]
    pub theme_subdir: Option<String>,
    #[arg(
        long,
        help = "Directory name created under themes/ for the downloaded theme",
        long_help = "Override the directory name used under themes/. Defaults to the bundled theme name."
    )]
    pub theme_name: Option<String>,
    #[arg(
        long,
        help = "Strip the given number of leading path components while extracting the archive",
        long_help = "Useful when the theme archive nests the files under multiple leading directories."
    )]
    pub strip_components: Option<usize>,
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
        about = "Apply a theme by copying its templates and assets",
        long_about = "Copy templates and static assets from the selected theme into the project directories and update bckt.yaml to reference it."
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
        about = "Download a theme archive into the local themes directory",
        long_about = "Fetch a theme from a GitHub repository or a direct zip URL and store it under themes/<name>."
    )]
    Download(ThemeDownloadArgs),
}

#[derive(Args, Clone, Debug)]
pub struct ThemeDownloadArgs {
    #[arg(help = "Name for the theme directory under themes/")]
    pub name: String,
    #[arg(
        long,
        help = "Direct zip URL to download the theme from",
        conflicts_with = "github",
        long_help = "Provide an HTTP(S) URL that points directly to a zip archive containing the theme files."
    )]
    pub url: Option<String>,
    #[arg(
        long = "github",
        help = "GitHub repository in the form owner/repo[/path]",
        conflicts_with = "url",
        long_help = "Fetch the theme from a GitHub repository. You can append an optional path (for example owner/repo/themes) to preselect a subdirectory. Combine with --branch or --tag and --subdir to override the folder if needed."
    )]
    pub github: Option<String>,
    #[arg(
        long,
        requires = "github",
        conflicts_with = "branch",
        help = "Git tag to download when using --github"
    )]
    pub tag: Option<String>,
    #[arg(
        long,
        requires = "github",
        conflicts_with = "tag",
        help = "Git branch to download when using --github"
    )]
    pub branch: Option<String>,
    #[arg(long, help = "Subdirectory inside the archive that contains the theme")]
    pub subdir: Option<String>,
    #[arg(
        long,
        help = "Strip the given number of leading path components while extracting the archive"
    )]
    pub strip_components: Option<usize>,
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
    #[arg(long, help = "Get the paginate_tags configuration value")]
    pub paginate_tags: bool,
    #[arg(long, help = "Get the default_timezone configuration value")]
    pub default_timezone: bool,
    #[arg(long, help = "Get the theme configuration value")]
    pub theme: bool,
    #[arg(long, help = "Get the search.asset_path configuration value")]
    pub search_asset_path: bool,
    #[arg(long, help = "Get the search.default_language configuration value")]
    pub search_default_language: bool,
}
