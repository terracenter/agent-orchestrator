# PROJECT_NAME

> [!IMPORTANT]
> Current status: describe whether the project is in design, development, production, or maintenance.

Short description in American English: what it solves, who it serves, and why it exists.

**License:** AGPL-3.0-or-later when applicable. If the project is offered as a network service, document the obligation to publish the corresponding source code.

## Architecture

| Layer | Technology | Reason |
|---|---|---|
| Backend | Go + Chi/net/http | Native, simple, auditable |
| UI | Go Templates + HTMX/Alpine + Tailwind | No heavy SPA runtime in production |
| Data | PostgreSQL/SQLite | Based on operational needs |
| Deploy | Go binary / minimal container | Smaller attack surface, easier rollback |

## Security

> [!WARNING]
> Do not commit secrets, tokens, real `.env` files, or database dumps.

Required minimums:

- secrets outside git;
- sensitive files with `600` permissions;
- documented auth/authorization;
- logs without credentials;
- input validation;
- rollback documented before production.

## Local development

```bash
make dev
```

## Validation

```bash
make test
make security
make build
```

## Documentation

- [Roadmap](ROADMAP.md)
- [Security](SECURITY.md)
- [Releases](RELEASES.md)

## License

See `LICENSE`.
