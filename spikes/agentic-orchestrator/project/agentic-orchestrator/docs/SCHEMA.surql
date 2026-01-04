-- SurrealDB schema (lightweight v0)
-- This is applied at daemon startup.

DEFINE NAMESPACE IF NOT EXISTS orchestrator;
DEFINE DATABASE IF NOT EXISTS orchestrator;

USE NS orchestrator;
USE DB orchestrator;

DEFINE TABLE intent SCHEMALESS;
DEFINE TABLE workflow SCHEMALESS;
DEFINE TABLE run SCHEMALESS;
DEFINE TABLE stage_run SCHEMALESS;
DEFINE TABLE job SCHEMALESS;
DEFINE TABLE artifact SCHEMALESS;

-- Dependency edges: stage_run ->requires-> stage_run
DEFINE TABLE requires SCHEMALESS;
