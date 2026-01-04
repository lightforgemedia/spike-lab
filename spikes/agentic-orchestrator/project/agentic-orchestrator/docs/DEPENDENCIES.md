# Dependencies (pinned for v0)

These are the versions pinned in `Cargo.toml` as of 2026-01-03:

- Rust MSRV: 1.80.1 (SurrealDB SDK requirement)
- surrealdb: 2.4.0 (embedded, `kv-surrealkv`)
- tokio: 1.48.0
- axum: 0.8.8
- tower-http: 0.6.8
- reqwest: 0.13.1 (agent HTTP client; rustls)
- serde: 1.0.228
- serde_json: 1.0.148
- thiserror: 2.0.17
- tracing: 0.1.44
- tracing-subscriber: 0.3.22
- uuid: 1.19.0
- ulid: 1.2.1
- clap: 4.5.53
- anyhow: 1.0.100

Update policy:
- bump periodically, keep MSRV aligned with SurrealDB.
