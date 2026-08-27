# Contributing

Thank you for contributing to `agent-orchestrator`.

## Languages

- Primary project language: Venezuelan Spanish.
- Secondary public documentation: American English.
- Public documentation changes should keep `README.md` and `README.en.md` aligned when applicable.

## Workflow

1. Create a `dev-<task>` branch from an up-to-date `main`.
2. Make small, verifiable commits.
3. Run tests before opening or updating a PR:

```bash
docker compose run --rm dev go test ./...
```

4. Open a Pull Request against `main`.
5. Wait for the `go-test` check to pass.

## Security

- Do not commit secrets, tokens, `.env`, SSH/GPG keys, or sensitive outputs.
- `main` is protected: no force pushes, no deletion, and changes go through PRs.
- Automatic agent execution is not part of the initial MVP.

## License

Contributions are accepted under AGPLv3 or later.
