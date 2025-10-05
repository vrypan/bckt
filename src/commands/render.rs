use anyhow::Result;
use std::env;

use crate::cli::RenderArgs;
use crate::config;
use crate::render::{BuildMode, RenderPlan, render_site};

pub fn run_render_command(args: RenderArgs) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = config::find_project_root(&cwd)?;
    let plan = determine_plan(args);
    render_site(&root, plan)
}

fn determine_plan(args: RenderArgs) -> RenderPlan {
    let posts = args.posts;
    let static_assets = args.static_assets;
    let mode = if args.force {
        BuildMode::Full
    } else {
        BuildMode::Changed
    };

    match (posts, static_assets) {
        (false, false) => RenderPlan {
            posts: true,
            static_assets: true,
            mode,
            verbose: args.verbose,
        },
        _ => RenderPlan {
            posts,
            static_assets,
            mode,
            verbose: args.verbose,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_defaults_to_both_when_flags_missing() {
        let plan = determine_plan(RenderArgs {
            posts: false,
            static_assets: false,
            force: false,
            verbose: false,
        });
        assert!(plan.posts);
        assert!(plan.static_assets);
        assert!(matches!(plan.mode, BuildMode::Changed));
        assert!(!plan.verbose);
    }

    #[test]
    fn plan_respects_individual_flags() {
        let plan = determine_plan(RenderArgs {
            posts: true,
            static_assets: false,
            force: false,
            verbose: false,
        });
        assert!(plan.posts);
        assert!(!plan.static_assets);
        assert!(matches!(plan.mode, BuildMode::Changed));
        assert!(!plan.verbose);

        let plan = determine_plan(RenderArgs {
            posts: false,
            static_assets: true,
            force: false,
            verbose: true,
        });
        assert!(!plan.posts);
        assert!(plan.static_assets);
        assert!(matches!(plan.mode, BuildMode::Changed));
        assert!(plan.verbose);
    }

    #[test]
    fn force_overrides_changed_mode() {
        let plan = determine_plan(RenderArgs {
            posts: false,
            static_assets: false,
            force: true,
            verbose: false,
        });
        assert!(matches!(plan.mode, BuildMode::Full));
    }
}
