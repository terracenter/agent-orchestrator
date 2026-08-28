.PHONY: dev test security build lint clean

dev:
	go run ./cmd/server

test:
	go test ./...
	go test -race ./...
	go vet ./...

security:
	govulncheck ./...
	gosec ./...

build:
	CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o bin/app ./cmd/server

lint:
	gofmt -w $$(find . -name '*.go' -not -path './vendor/*')
	go vet ./...

clean:
	rm -rf bin
