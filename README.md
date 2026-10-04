# Custodian

**Tidy your computer without fear. Every change comes with a way back.**

Custodian is a local, open-source AI companion for Windows 11. It understands
what is on your disk, frees space safely, tests every change in a sandbox
first, keeps all data on your machine, and proves every undo with a checksum.
A small on-screen companion, **Cubby**, is the user interface.

> **Status:** early development. Many components are placeholders with their
> final interfaces; [`HONESTY.md`](HONESTY.md) lists exactly what is stubbed.

## Architecture

Each process runs with the least privilege it needs and talks to the others
over HMAC-authenticated IPC frames (`custodian-ipc`).

| Component     | Path                                             | Language        | Role                                                  |
| ------------- | ------------------------------------------------ | --------------- | ----------------------------------------------------- |
| Desktop shell | [`apps/desktop`](apps/desktop)                   | Tauri 2 + React | Cubby window, tray, approvals                         |
| Core & engine | [`crates/`](crates)                              | Rust            | Supervision, journaled and reversible file operations |
| Agent         | [`services/agent`](services/agent)               | TypeScript      | Prompt loading, tool schemas, output verification     |
| Broker        | [`services/broker`](services/broker)             | TypeScript      | Isolated browser actions for online mode              |
| Orchestrator  | [`services/orchestrator`](services/orchestrator) | TypeScript      | Temporal worker for long-running workflows            |
| ML host       | [`services/mlhost`](services/mlhost)             | Python          | Local model inference (Laya) over stdio IPC           |
| Wire schemas  | [`proto/`](proto)                                | JSON Schema     | Source of truth for shared types                      |

### Rust crates

| Crate                | Purpose                                                         |
| -------------------- | --------------------------------------------------------------- |
| `custodian-core`     | Process supervisor, IPC routing policy, install-lab helpers     |
| `custodian-engine`   | Approval verification, reversible operations, recovery, undo    |
| `custodian-journal`  | File-system watcher, debouncer, CRC-checked write-ahead journal |
| `custodian-ledger`   | Hash-chained, tamper-evident transaction ledger                 |
| `custodian-ipc`      | Authenticated framing and per-role key matrix                   |
| `custodian-winsec`   | `SafePath`, AppContainer, job objects, DPAPI                    |
| `custodian-digest`   | Streaming BLAKE3 file digests                                   |
| `custodian-store`    | PostgreSQL persistence (transactions, pgvector, TimescaleDB)    |
| `custodian-proto`    | Shared wire types                                               |
| `custodian-parser`   | Sandboxed document parser                                       |
| `custodian-sandbox`  | Sandbox launcher and declared-effect checks                     |
| `custodian-elevate`  | Minimal elevation helper with a fixed verb allow-list           |
| `custodian-fixtures` | Generates marker-protected fixture trees                        |
| `custodian-testkit`  | Test helpers that refuse to touch real user folders             |

## Repository layout

```
apps/desktop/        Tauri desktop shell (React frontend + src-tauri)
crates/              Rust workspace crates
services/            Node.js services and the Python ML host
proto/               JSON schemas for shared types
tests/               Cross-component end-to-end and security tests
training/            Synthetic dataset generator for the Laya model
scripts/             PowerShell helpers (models, codegen, PE checks)
models/              Local model files (git-ignored)
docs/                Design documents, verification log, versions
```

## Getting started

### Prerequisites

- Rust 1.97 (installed automatically from `rust-toolchain.toml`)
- Node.js 24 (see `.nvmrc`)
- Python 3.12
- Windows 11 for the full product; the libraries and tests also run on Linux
  (the Tauri shell needs the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

### Build and test

```sh
npm ci                                   # JavaScript dependencies
npm run build -w apps/desktop            # frontend, embedded by the Tauri build
cargo test --workspace                   # Rust
npm test                                 # TypeScript (vitest)

pip install -r services/mlhost/requirements.txt -r requirements-dev.txt
pytest                                   # Python
```

### Run

```sh
npm run tauri -w apps/desktop -- dev     # desktop shell with hot reload
CUSTODIAN_IPC_KEY=dev-key PYTHONPATH=services/mlhost python -m mlhost
npm run fixtures -- --persona demo --out ./fixture   # sample data
```

App icons are generated from `apps/desktop/src-tauri/icons/custodian.svg`. After
editing it, run `npm run icons -w apps/desktop` from the repository root.

Place model files in `models/` with `scripts/fetch-models.ps1`, which also pins
their SHA-256 in `models/manifest.json`; unpinned or modified models are refused.

## Quality checks

CI runs the same commands on every pull request:

| Area       | Commands                                                                                               |
| ---------- | ------------------------------------------------------------------------------------------------------ |
| Rust       | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |
| TypeScript | `npm run format:check`, `npm run lint`, `npm run typecheck`, `npm test`                                |
| Python     | `ruff check .`, `ruff format --check .`, `mypy .`, `pytest`                                            |
| Security   | `cargo deny check`, `npm audit`, `pip-audit`, `python tests/security/scan_secrets.py`                  |

## Documentation

- [Master document](docs/design/CUSTODIAN_MASTER_DOCUMENT.md): what we build and why
- [Architecture](docs/design/CUSTODIAN_ARCHITECTURE.md): how it is built
- [Build plan](docs/design/CUSTODIAN_BUILD_PLAN.md): the ordered build steps
- [Verification log](docs/VERIFY-LOG.md) and [component versions](docs/VERSIONS.md)

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md). Report vulnerabilities privately as
described in [SECURITY.md](SECURITY.md).

## License

[Apache License 2.0](LICENSE)
