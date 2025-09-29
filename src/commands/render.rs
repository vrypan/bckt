use anyhow::Result;
use std::env;

use crate::cli::RenderArgs;
use crate::render::{RenderPlan, render_site};

pub fn run_render_command(args: RenderArgs) -> Result<()> {
    let root = env::current_dir()?;
    let plan = determine_plan(args);
    render_site(&root, plan)
}

fn determine_plan(args: RenderArgs) -> RenderPlan {
    let posts = args.posts;
    let static_assets = args.static_assets;

    match (posts, static_assets) {
        (false, false) => RenderPlan {
            posts: true,
            static_assets: true,
        },
        _ => RenderPlan {
            posts,
            static_assets,
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
        });
        assert!(plan.posts);
        assert!(plan.static_assets);
    }

    #[test]
    fn plan_respects_individual_flags() {
        let plan = determine_plan(RenderArgs {
            posts: true,
            static_assets: false,
        });
        assert!(plan.posts);
        assert!(!plan.static_assets);

        let plan = determine_plan(RenderArgs {
            posts: false,
            static_assets: true,
        });
        assert!(!plan.posts);
        assert!(plan.static_assets);
    }
}
