.PHONY: test build install clean

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
BIN ?= orq

GO ?= go

build:
	$(GO) build -buildvcs=false -o bin/$(BIN) ./cmd/orq

test:
	$(GO) test ./...

install:
	@tmpdir=$$(mktemp -d 2>/dev/null || mktemp -d -t 'orq-install'); \
	trap 'rm -rf "$$tmpdir"' EXIT INT TERM; \
	$(GO) build -buildvcs=false -o "$$tmpdir/$(BIN)" ./cmd/orq && \
	mkdir -p $(BINDIR) && \
	install -m 0755 "$$tmpdir/$(BIN)" $(BINDIR)/$(BIN)

clean:
	rm -rf bin
