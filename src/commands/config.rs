use anyhow::{Result, bail};
use std::env;

use crate::cli::ConfigArgs;
use crate::config::{find_project_root, Config};

pub fn run_config_command(args: ConfigArgs) -> Result<()> {
    // Find project root from current directory
    let cwd = env::current_dir()?;
    let root = find_project_root(&cwd)?;

    // Handle --root flag
    if args.root {
        println!("{}", root.display());
        return Ok(());
    }

    // Load config
    let config_path = root.join("bckt.yaml");
    let config = Config::load(&config_path)?;

    // Count how many flags are set
    let flags_set = [
        args.base_url,
        args.title,
        args.homepage_posts,
        args.date_format,
        args.paginate_tags,
        args.default_timezone,
        args.theme,
        args.search_asset_path,
        args.search_default_language,
    ]
    .iter()
    .filter(|&&flag| flag)
    .count();

    // Ensure exactly one flag is specified
    if flags_set == 0 {
        bail!("No config key specified. Use --help to see available options.");
    }
    if flags_set > 1 {
        bail!("Only one config key can be queried at a time.");
    }

    // Output the requested value
    if args.base_url {
        println!("{}", config.base_url);
    } else if args.title {
        if let Some(title) = &config.title {
            println!("{}", title);
        }
    } else if args.homepage_posts {
        println!("{}", config.homepage_posts);
    } else if args.date_format {
        println!("{}", config.date_format);
    } else if args.paginate_tags {
        println!("{}", config.paginate_tags);
    } else if args.default_timezone {
        println!("{}", config.default_timezone);
    } else if args.theme {
        if let Some(theme) = &config.theme {
            println!("{}", theme);
        }
    } else if args.search_asset_path {
        println!("{}", config.search.asset_path);
    } else if args.search_default_language {
        println!("{}", config.search.default_language);
    }

    Ok(())
}
