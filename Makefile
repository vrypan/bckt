.PHONY: build themes dev-themes test

# Build the bckt binaries.
build:
	cargo build --release

# Package every theme under themes/ into target/themes/<name>.zip.
themes:
	./scripts/package-themes.sh

# Copy all themes into target/debug/themes/ and demo content into
# target/debug/demo/ so `bckt init` finds them on the search path during
# local development.
dev-themes:
	cargo build
	mkdir -p target/debug/themes target/debug/demo
	for d in themes/*/; do cp -r "$$d" target/debug/themes/; done
	for d in demo/*/; do cp -r "$$d" target/debug/demo/; done

test:
	cargo test
