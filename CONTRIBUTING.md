# Contributing to Nodikle

Thank you for your interest in contributing to **Nodikle**!  
Whether you are reporting a bug, proposing an architectural improvement, refining documentation, or submitting a pull request, your contributions help make Nodikle a fast, reliable, local-first workspace across Desktop and Mobile.

Please take a moment to review these guidelines before making your first contribution.

> **Key Reference Documents**
> - [README.md](README.md) — Project overview, architecture summary, and roadmap
> - [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — Community standards
> - [01. System Architecture & UI Strategy](docs/01-architecture.md)
> - [02. Hybrid Data Model & Database Views](docs/02-data-model-and-views.md)
> - [03. Extensible Storage & Cloud Sync](docs/03-storage-and-sync.md)
> - [04. Implementation Roadmap](docs/04-roadmap.md)
> - [CHANGELOG.md](CHANGELOG.md)

---

## Table of Contents

1. [Code of Conduct](#1-code-of-conduct)
2. [Reporting Issues & Security Concerns](#2-reporting-issues--security-concerns)
3. [Planned Development Environment](#3-planned-development-environment)
4. [Planned Workspace Structure](#4-planned-workspace-structure)
5. [Code Style & Cross-Platform Guidelines](#5-code-style--cross-platform-guidelines)
6. [Testing & Local Verification](#6-testing--local-verification)
7. [Git Workflow, Commits & Pull Requests](#7-git-workflow-commits--pull-requests)
8. [Versioning Policy](#8-versioning-policy)
9. [License](#9-license)

---

## 1. Code of Conduct

All contributors and maintainers are expected to abide by our [Code of Conduct](CODE_OF_CONDUCT.md) across all project spaces. Please read it before participating in discussions, issues, or pull requests.

---

## 2. Reporting Issues & Security Concerns

Before opening a new issue, please search existing issues to check whether the problem or suggestion has already been reported.

- **Bug Reports (`[BUG]`)**: Use the [Bug Report template](.github/ISSUE_TEMPLATE/bug_report.md).
  - Include your OS/architecture (Windows / macOS / Linux / Android), Nodikle version, storage or sync backend (`Local Vault`, `Android App Storage`, `Android SAF`, `Google Drive`), reproduction steps, expected vs. actual behavior, and relevant logs.
  - **Important**: Always redact OAuth tokens, refresh tokens, API secrets, and personal note contents before attaching logs or screenshots.
- **Feature Requests (`[FEAT]`)**: Use the [Feature Request template](.github/ISSUE_TEMPLATE/feature_request.md).
  - Clearly describe the motivation, proposed solution, alternatives considered, and acceptance criteria.
- **Security Issues**: Please **do not** open public GitHub issues with exploit details for vulnerabilities affecting OAuth PKCE flows, token/secret storage, Android SAF permissions, or local vault integrity. Instead, report them privately to the repository maintainer (`@Othkkartho`) via GitHub.

---

## 3. Planned Development Environment

Nodikle is currently in **Phase 0 (Architecture & Documentation)**. Once the Rust workspace and frontend scaffold are introduced in Phase 1+, the target toolchain will be:

| Tool | Minimum Version | Purpose |
|---|---|---|
| **Rust** (`rustup`, `cargo`) | `1.80+` | Core workspace crates, SQLite indexer, sync engine, and unit tests |
| **Node.js** (`npm`) | `22+` | Frontend (`frontend/`) build and tooling |
| **Android SDK / NDK** | API 26+ (Android 8.0+) | Android MVP builds via Tauri v2 Mobile |

**OS-Specific Tauri v2 Dependencies:**
- **Windows**: Microsoft Edge WebView2 Runtime, Visual Studio Build Tools (Desktop development with C++)
- **macOS**: Xcode Command Line Tools (`xcode-select --install`)
- **Linux (Ubuntu/Debian)**: `build-essential`, `pkg-config`, `libssl-dev`, `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libsecret-1-dev`

---

## 4. Planned Workspace Structure

```text
nodikle/
├── Cargo.toml                  # Cargo workspace root (planned in Phase 1)
├── .github/
│   ├── ISSUE_TEMPLATE/         # Bug report and feature request templates
│   └── pull_request_template.md
├── crates/
│   ├── nodikle-model/          # Shared domain models and DTOs
│   ├── nodikle-core/           # Markdown/Frontmatter parser, links, tags, templates
│   ├── nodikle-index/          # SQLite + FTS5 indexer and query/aggregation engine
│   ├── nodikle-storage/        # LocalFileStore & SecretStore traits + platform adapters
│   └── nodikle-sync/           # CloudStorageProvider trait, OAuth PKCE, Google Drive sync
├── apps/
│   └── app/                    # Tauri v2 Desktop + Android/iOS host & native bridges
├── frontend/                   # TypeScript + React + ProseMirror/TipTap + CodeMirror 6 UI
└── docs/                       # Architecture, data model, storage/sync & roadmap docs
```

---

## 5. Code Style & Cross-Platform Guidelines

- **Google Style Guides**: Follow the [Google Style Guides](https://google.github.io/styleguide/) conventions for Rust, TypeScript/JavaScript, HTML/CSS, and Kotlin/Swift.
- **Formatting & Linting**:
  - Run `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets` before submitting Rust changes.
  - Keep `crates/nodikle-*` strictly platform-independent unless isolated behind trait implementations or `cfg` boundaries in `nodikle-storage` / `apps/app`.
- **Documentation Integrity**: Preserve existing comments and docstrings unless your changes directly affect them, and keep `docs/` and `CHANGELOG.md` updated when behavior or schemas change.

---

## 6. Testing & Local Verification

Every new feature or bug fix should include corresponding unit tests. Before opening a Pull Request, verify all applicable checks locally:

```bash
# Rust workspace checks and tests (Phase 1+)
cargo check --workspace
cargo test --workspace

# Frontend type-check and production build (Phase 2+)
npm --prefix frontend run build
```

---

## 7. Git Workflow, Commits & Pull Requests

- **Default Branch (`master`)**: Direct pushes to `master` are not permitted after initial repository setup. Always branch from `master` for new work (e.g., `feat/core-frontmatter-parser`, `fix/sync-conflict-copy`, `docs/update-roadmap`).
- **Commit Messages**: Follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) (e.g., `feat: add YAML frontmatter round-trip updater`, `fix: handle unresolved wikilinks in backlink index`, `docs: clarify Android SAF streaming limits`, `test: add FTS5 trigram search unit tests`).
- **Push Cadence**: Group logical changes into clean commits locally and push the completed branch once, rather than pushing every individual commit immediately.
- **Pull Requests**:
  - Target `master`.
  - Write the PR **Title** and **Description** in **English**, strictly following [.github/pull_request_template.md](.github/pull_request_template.md).

---

## 8. Versioning Policy

Nodikle follows [Semantic Versioning 2.0.0](https://semver.org/) (`MAJOR.MINOR.PATCH`):
- **MAJOR**: Incremented only when explicitly requested by the maintainer/user for breaking changes.
- **MINOR**: Incremented for backward-compatible feature additions.
- **PATCH**: Incremented for backward-compatible bug fixes, refactoring, or documentation updates.

---

## 9. License

By contributing to Nodikle, you agree that your contributions will be licensed under the [MIT License](LICENSE).
