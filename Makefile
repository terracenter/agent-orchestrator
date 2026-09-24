.PHONY: test build install clean

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
CONFIG_DIR ?= $(HOME)/Workspace/.agents/orq
BIN ?= orq
CARGO ?= cargo
CARGO_MANIFEST ?= orq-agent/Cargo.toml

build:
	$(CARGO) build --release --manifest-path $(CARGO_MANIFEST) --bins

test:
	$(CARGO) test --manifest-path $(CARGO_MANIFEST)

install:
	$(CARGO) build --release --manifest-path $(CARGO_MANIFEST) --bins
	mkdir -p $(BINDIR)
	mkdir -p $(CONFIG_DIR)
	install -m 0755 orq-agent/target/release/orq $(BINDIR)/$(BIN)
	install -m 0755 orq-agent/target/release/orq-agent $(BINDIR)/orq-agent
	for config in orq-agent/config/*.json; do \
		target="$(CONFIG_DIR)/$$(basename "$$config")"; \
		if [ -e "$$target" ]; then cp -p "$$target" "$$target.backup.$$(date -u +%Y%m%dT%H%M%SZ)"; fi; \
		install -m 0644 "$$config" "$$target"; \
	done

clean:
	rm -rf bin
