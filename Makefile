.PHONY: test build install clean

PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
BIN ?= orq

GO ?= go

build:
	$(GO) build -buildvcs=false -o bin/$(BIN) ./cmd/orq

test:
	$(GO) test ./...

install: build
	mkdir -p $(BINDIR)
	install -m 0755 bin/$(BIN) $(BINDIR)/$(BIN)

clean:
	rm -rf bin
