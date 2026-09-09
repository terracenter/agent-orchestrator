.PHONY: test build install clean

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
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
	install -m 0755 orq-agent/target/release/orq $(BINDIR)/$(BIN)
	install -m 0755 orq-agent/target/release/orq-agent $(BINDIR)/orq-agent

clean:
	rm -rf bin
