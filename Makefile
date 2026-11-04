BINARY := dstack
PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin
ARGS ?=

.PHONY: build release run install update uninstall clean check test fmt fmt-check lint v1-test v1-pty ci

build:
	cargo build --locked

release:
	cargo build --release --locked

run:
	cargo run --locked -- $(ARGS)

# Source-install convenience for v2 development. This is not the future
# repository-owned production installer described by the architecture docs.
install: release
	mkdir -p "$(BINDIR)"
	install -m 755 "target/release/$(BINARY)" "$(BINDIR)/$(BINARY)"

update:
	git pull --ff-only
	$(MAKE) install

uninstall:
	rm -f "$(BINDIR)/$(BINARY)"

clean:
	cargo clean

check:
	cargo check --locked

test:
	cargo test --all-features --locked

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --locked --all-targets --all-features -- -D warnings

v1-test:
	bash tests/characterization/run.sh all

v1-pty:
	bash tests/characterization/run.sh interactive

ci: fmt-check lint test v1-test
