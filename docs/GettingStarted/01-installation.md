# Installation

This guide will help you install bckt and its companion tools on your system.

## Installation Methods

### Option 1: Download Pre-built Binaries (Recommended)

The easiest way to get started:

1. Visit the [bckt releases page](https://github.com/vrypan/bckt/releases)
2. Download the latest release for your platform
3. Extract the archive
4. Move the binaries to a directory in your `PATH`:

```bash
# Example for Unix-like systems
mv bckt /usr/local/bin/
mv bckt-new /usr/local/bin/

# Make them executable if needed
chmod +x /usr/local/bin/bckt
chmod +x /usr/local/bin/bckt-new
```

### Option 2: Homebrew

If you're on macOS and use Homebrew:

```bash
brew tap vrypan/bckt
brew install bckt
```

This will install both `bckt` and `bckt-new`.

### Option 3: cargo

If you have Rust installed, you can install bckt using cargo:

```bash
cargo install bckt
```

Alternatively, you can build from the repository:

```bash
git clone https://github.com/vrypan/bckt.git
cd bckt
cargo install --path .

# Install companion tools
cd bckt-new
cargo install --path .
```

## Verify Installation

Check that bckt is installed correctly:

```bash
bckt --version
```

You should see output like: `bckt 0.6.2`

## Next Steps

Now that bckt is installed, you're ready to create your first blog!

Continue to: [Setting Up Your First Blog](02-first-blog.md)
