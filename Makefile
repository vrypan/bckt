.PHONY: build themes dev-themes test

# Build the bckt binaries.
build:
	cargo build --release

# Package every theme under themes/ into target/themes/<name>.zip.
themes:
	./scripts/package-themes.sh

# Package themes and drop the default bckt3.zip next to the debug binary so
# `bckt init` finds it on the search path during local development.
dev-themes: themes
	cargo build
	cp target/themes/bckt3.zip target/debug/bckt3.zip

test:
	cargo test
