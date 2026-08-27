# Security policy

## Branch protection

The public repository protects `main` with a GitHub ruleset named `protect-main`.

Required behavior:

- no direct updates to `main` outside Pull Requests;
- no force pushes to `main`;
- no deletion of `main`;
- Pull Requests require approval;
- Pull Requests require the `go-test` status check.

Verification:

```bash
gh api repos/terracenter/agent-orchestrator/rulesets
```

## Reporting security issues

Do not open public issues with secrets, tokens, credentials, or exploit details. Contact the maintainer privately first.
