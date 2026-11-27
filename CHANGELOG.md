# Changelog

All notable changes to **Nodikle** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-27

### Added
- **Phase 1 — Rust Core & Local Index Engine (`crates/`)**:
  - `nodikle-model`: Shared DTOs, typed property values, database view schemas, filter AST, and IPC error taxonomy.
  - `nodikle-core`: Markdown parser (`pulldown-cmark`), lossless YAML frontmatter round-trip patcher, `[[wikilink]]` and nested `#tag` extraction ignoring code blocks, task line toggle, template engine (`{{date}}`, `{{title}}`, `{{uuid}}`), and starter vault templates/schemas.
  - `nodikle-storage`: `LocalFileStore` and `SecretStore` traits, path traversal validation, and atomic `DesktopFsStore` with optimistic locking (`BLAKE3`).
  - `nodikle-index`: SQLite WAL + FTS5 (`trigram`) search with short-query `LIKE` fallback, incremental scanner, backlink resolution, and parameterized view query/aggregation engine.
- **Phase 2 — Tauri v2 App Host & Hybrid UI (`apps/app`, `frontend/`)**:
  - `nodikle-app`: Tauri v2 desktop/mobile-ready host, typed IPC commands, and debounced desktop file watcher with self-write echo suppression.
  - `frontend`: React + TypeScript workspace featuring a hybrid raw Markdown (CodeMirror 6) / block (TipTap) editor, frontmatter property inspector, `[[` wikilink autocomplete, backlinks panel, Table/Board/Calendar database views, Expense Tracker & Trip Planner starter schemas, mobile card view auto-switching, and Quick Capture bottom sheet.
- **CI/CD Workflows**:
  - PR-only CI workflow (`.github/workflows/ci.yml`) running frontend build/tests and cross-platform Rust fmt/clippy/tests.
  - Version-change-gated release workflow (`.github/workflows/release.yml`) publishing Windows, macOS, and Linux archives when `master` advances with a new workspace version.
- Initial project overview, architecture, data model, storage/sync, and roadmap documentation.
