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
