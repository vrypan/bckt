# bucket3rs

`bucket3rs` is a static site generator that builds a microblog-friendly HTML tree.

## Development

```
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Usage

```
bucket3 init
```

The `init` command creates the starter structure: `html/`, `posts/`, `templates/`, `skel/`, and a `bucket3.yaml` configuration file. The command is idempotent and prints `Initialized` when the workspace is ready.

### Configuration

`bucket3.yaml` drives site-wide settings. All fields are optional; missing values fall back to:

```
base_url: "https://example.com"
homepage_posts: 5
date_format: "[year]-[month]-[day]"
```

`base_url` must be an absolute `http` or `https` URL, `homepage_posts` must be positive, and `date_format` accepts either a custom [`time` format description`](https://docs.rs/time/latest/time/format_description/) or the keyword `RFC3339`. The configuration is injected into templates as `config`, and templates can call `{{ now() }}` (or `{{ now('RFC3339') }}`) to render the current timestamp.
