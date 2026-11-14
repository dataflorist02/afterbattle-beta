# Contributing

Contributions are welcome after the local alpha has validated the API schema.

Before opening a pull request:

1. Do not include API keys, player-private data, real raw payloads, or database files.
2. Add sanitized fixtures for API-normalization changes.
3. Run `pnpm test`, `pnpm build`, `cargo fmt --check`, and `cargo test`.
4. Explain any new dependency and its runtime permissions.
5. Update the threat model for changes to IPC, networking, persistence, secrets, export, replay parsing, or updates.
6. Keep feedback rules deterministic, source-visible, and supported by measurable inputs.

Security issues must follow [SECURITY.md](SECURITY.md), not public issue reporting.

