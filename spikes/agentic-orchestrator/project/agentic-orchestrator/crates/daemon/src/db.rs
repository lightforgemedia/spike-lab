use std::path::Path;

use surrealdb::{engine::any::connect, Surreal};

/// Database wrapper for embedded SurrealDB.
#[derive(Clone)]
pub struct Db {
    inner: Surreal<surrealdb::engine::any::Any>,
}

impl Db {
    /// Connect to an embedded SurrealKV datastore at `db_dir`.
    pub async fn connect(db_dir: &Path) -> anyhow::Result<Self> {
        // SurrealDB Any engine chooses the engine by endpoint scheme.
        // With `kv-surrealkv` enabled, we can use `surrealkv://...`.
        let endpoint = format!("surrealkv://{}", db_dir.display());
        let db = connect(endpoint).await?;
        db.use_ns("orchestrator").use_db("orchestrator").await?;
        Ok(Self { inner: db })
    }

    pub fn inner(&self) -> &Surreal<surrealdb::engine::any::Any> {
        &self.inner
    }

    /// Apply schema at startup.
    pub async fn apply_schema(&self) -> anyhow::Result<()> {
        // Lightweight schema (tables + edge table). Idempotent for v0.
        let schema = include_str!("../../../docs/SCHEMA.surql");
        self.inner.query(schema).await?;
        Ok(())
    }
}
