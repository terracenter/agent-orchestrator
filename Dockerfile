FROM rust:1.97

WORKDIR /workspace

COPY . .

CMD ["cargo", "test", "--manifest-path", "orq-agent/Cargo.toml"]
