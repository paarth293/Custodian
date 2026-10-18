# Custodian

**Tidy your computer without fear. Every change comes with a way back.**

Custodian is a local, open-source AI companion for Windows 11, with **Cubby**
as its on-screen interface. The current offline foundation supports permitted
local-file discovery and approved file operations with verification and undo.
Private-location controls are implemented for supported file tools; general
desktop automation and full security acceptance remain unfinished.

> **Status:** early development. Many components are placeholders with their
> final interfaces; [`HONESTY.md`](HONESTY.md) lists exactly what is stubbed.

The [capability register](docs/CAPABILITY-REGISTER.md) tracks the full offline
vision and gaps. The [Stage 0 baseline](docs/STAGE-0-BASELINE-2026-10-17.md)
records source/runtime identities, frozen expectations and validation.
The [Stage 0 runtime repair](docs/STAGE-0-RUNTIME-REPAIR-2026-10-17.md) records
the restored baseline and two fresh desktop passes.
The [Stage 1 authority and recovery report](docs/STAGE-1-AUTHORITY-AND-RECOVERY-2026-10-18.md)
records shared native authorization, optional copy-grant scopes, one active host
per user across profiles, real process-kill recovery and 27 passing checks on the
optimized desktop. It identifies that increment's native artifact; the installer
remains the historical Stage 0 build. Full-product security acceptance is open.

The [Stage 2 coordinator report](docs/STAGE-2-TASK-COORDINATOR-2026-10-18.md)
records bounded typed programs, durable step evidence, safe pause, failure
classification and verified collection integration. App composition uses a
labelled mock in fixtures; real application control and general language
planning remain separate unfinished stages.

The [Stage 3 language/planning report](docs/STAGE-3-LANGUAGE-AND-PLANNING-2026-10-18.md)
records local Qwen interpretation of file collections and location roles,
independent native validation, real model-to-file execution and the current
desktop acceptance status. The supported combinations are files/documents/images,
optionally selecting the largest. General app automation, content-based
exclusions and multilingual execution remain unfinished; Laya stays inactive.

The [Stage 4 foundation review](docs/STAGE-4-FOUNDATION-REVIEW-2026-10-18.md)
records the current app-contract and admission work.
The [supervised worker report](docs/STAGE-4-SUPERVISED-WORKER-2026-10-18.md)
adds a one-shot native worker with authenticated IPC, enforced deadlines and
bounded cleanup. The [persistent app privacy increment](docs/STAGE-4-APP-PRIVACY-2026-10-18.md)
adds encrypted exclusions and desktop controls. The subsequent
[real application implementation](docs/STAGE-4-REAL-APP-IMPLEMENTATION-2026-10-18.md)
adds bound VS Code text editing, a supervised Excel adapter, verified publication,
undo and encrypted recovery evidence. These are version-qualified, task-owned
document workflows; general desktop observation remains disabled. Stage 4
acceptance and installation status are recorded in that report.

The [Stage 5 execution record](docs/STAGE-5-EXECUTION-RECORD-2026-10-18.md)
records bounded batch manifests, ZIP creation/extraction, versioned backup and
restore, and governed recycle core increments. These primitives have offline
tests, but Stage 5 is not accepted: desktop routes, a qualified Windows
Recycle Bin provider, and optimized crash/recovery/package evidence remain
open.

The [Stage 6 execution record](docs/STAGE-6-EXECUTION-RECORD-2026-10-18.md)
records the bounded, offline TXT/Markdown/CSV content core and its typed
provenance and verification contracts. DOCX, PDF, PPTX, image, OCR and
audio/video capabilities remain unavailable until their workers, dependencies,
assets and independent verifiers are qualified; Stage 6 is not accepted.

See the [offline foundation report](docs/OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md)
for baseline scope, paused capabilities and remaining gates.
The [routine backup increment](docs/ROUTINE-BACKUP-IMPLEMENTATION-2026-10-17.md)
adds opt-in automatic discovery, selection, verified copying and undo for one
explicit largest-file goal across permitted local folders or all local drives.
It includes that increment's historical hashes and two desktop walkthroughs.
General autonomous desktop operation remains unfinished.

The [model-planned workflow increment](docs/MODEL-PLANNED-WORKFLOWS-2026-10-17.md)
adds document/photo collections and composed largest-file selection through one
shared engine, with a separate standing permission, saved step receipts and
group undo. Qwen proposes bounded capability sequences; native code validates
the original goal and paths before execution. This remains constrained file
automation, with no archive creation or general application control. Its artifact
hashes are historical; the Stage 0 repair report identifies the current installer.

## Architecture

Each process runs with the least privilege it needs and talks to the others
over HMAC-authenticated IPC frames (`custodian-ipc`).

| Component     | Path                                             | Language        | Role                                                  |
| ------------- | ------------------------------------------------ | --------------- | ----------------------------------------------------- |
| Desktop shell | [`apps/desktop`](apps/desktop)                   | Tauri 2 + React | Cubby window, tray, approvals                         |
| Core & engine | [`crates/`](crates)                              | Rust            | Supervision, journaled and reversible file operations |
| Agent         | [`services/agent`](services/agent)               | TypeScript      | Local chat model, prompts, output verification        |
| Broker        | [`services/broker`](services/broker)             | TypeScript      | Isolated browser actions for online mode              |
| Orchestrator  | [`services/orchestrator`](services/orchestrator) | TypeScript      | Temporal worker for long-running workflows            |
| ML host       | [`services/mlhost`](services/mlhost)             | Python          | Local model inference (Laya) over stdio IPC           |
| Wire schemas  | [`proto/`](proto)                                | JSON Schema     | Source of truth for shared types                      |

### Rust crates

| Crate                | Purpose                                                         |
| -------------------- | --------------------------------------------------------------- |
| `custodian-core`     | Duplicate finder, safe quarantine with undo, supervisor         |
| `custodian-core::content` | Bounded TXT/Markdown/CSV provenance, parsing and staged verification |
| `custodian-engine`   | Approval verification, reversible operations, recovery, undo    |
| `custodian-journal`  | File-system watcher, debouncer, CRC-checked write-ahead journal |
| `custodian-ledger`   | Hash-chained, tamper-evident transaction ledger                 |
| `custodian-ipc`      | Authenticated framing and per-role key matrix                   |
| `custodian-winsec`   | `SafePath`, AppContainer, job objects, DPAPI                    |
| `custodian-appworker` | Authenticated native probes and supervised, bound Excel sessions |
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
npm run dev -w apps/desktop              # then open http://localhost:1420/?gallery to preview every Cubby mood
CUSTODIAN_IPC_KEY=dev-key PYTHONPATH=services/mlhost python -m mlhost
npm run fixtures -- --persona demo --out ./fixture   # sample data
```

### Talking to Cubby

Click Cubby to open the chat. These requests work without any model:

| Ask                                                     | What happens                                                                                     |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| "where's my resume?", "open my resume"                  | Finds files by name in your folders; opens documents and media (never programs)                  |
| "open spotify", "open downloads", "open myntra"         | Opens any Start-menu app, a known folder or a website                                            |
| "close notepad", "what's open?"                         | Asks an app to close, as its X button does; lists open apps                                      |
| "remind me to call mom at 5pm", "add milk to my list"   | Reminders and a to-do list, sealed on this PC; "what's on my list?", "mark 1 done"               |
| "make windows dark", "show file extensions"             | A card shows exactly what will change and how to put it back; nothing is written until you click |
| "what settings can you change?", "put my theme back"    | Lists the settings Cubby may change on this PC, and puts back the last change it made            |
| "any suggestions?", "is my PC slow?", "how much space?" | A quick check-up with what to do next; CPU, memory, battery; free space per drive                |
| "find duplicates", "duplicates in Pictures"             | Proposes moving extra copies to the safe place (never inside OneDrive)                           |
| "clean temp files"                                      | Proposes deleting temporary files older than a week                                              |
| _Approve_ on a plan                                     | Confirmed with Windows Hello, then carried out and recorded                                      |
| "undo", "what did you do?", "empty the safe place"      | Puts the last clean-up back; lists clean-ups; frees the safe place's space now                   |
| "is my data safe?"                                      | Shows how Cubby protects your data, as it stands on this PC                                      |
| "search cheap flights", "what time is it?"              | Web search in your browser; time and date                                                        |

Copies moved to the safe place stay there for 30 days, then their space is
freed after each copy is checked against its original. When the AI asks to
do something that sends data off the PC or changes something, Cubby shows a
card and waits for your click. Press **Stop** to end any scan or answer.

Anything else goes to the local language model. To enable it, fetch a chat
model into `models/` (which pins its SHA-256) and build the agent once. Qwen3 4B
Instruct 2507 runs on the graphics card when there is one; Qwen2.5 1.5B Instruct
stands in for it on PCs without one, where the 4B model would take most of a
minute per answer:

```powershell
./scripts/fetch-models.ps1 -Repo unsloth/Qwen3-4B-Instruct-2507-GGUF -File Qwen3-4B-Instruct-2507-Q4_K_M.gguf -Default
./scripts/fetch-models.ps1 -Repo Qwen/Qwen2.5-1.5B-Instruct-GGUF -File qwen2.5-1.5b-instruct-q4_k_m.gguf
npm run build                           # creates services/agent/dist/server.js
```

Optional environment variables: `CUSTODIAN_CHAT_MODEL` (file name in `models/`;
always used when set), `CUSTODIAN_MODELS_DIR`, `CUSTODIAN_AGENT_ENTRY`,
`CUSTODIAN_NODE` (path to `node`) and `CUSTODIAN_LLAMA_GPU` (`cuda`, `vulkan`,
`metal`, or `off` for the CPU; by default the first GPU that works is used).
Without a GPU, the smallest pinned chat model is used in place of a default over
1.5 GB. Without a model, Cubby says so and lists what it can already do.

On Windows the AI runs inside an AppContainer: no network, and read access only
to Node, the agent's code and packages, and `models/`. The first start grants
that read access (about 15 seconds for `node_modules`). If the model cannot load
inside the container, AI startup fails closed and deterministic desktop tools
remain available. `CUSTODIAN_AGENT_SANDBOX=0` explicitly enables unrestricted
development startup.

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

The offline Windows installer requires a preinstalled Microsoft Edge WebView2
Runtime. It does not download that prerequisite automatically. Clean-profile
installation acceptance remains open; see the current implementation report.

- [Current offline implementation and limits](docs/OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md)

- [Master document](docs/design/CUSTODIAN_MASTER_DOCUMENT.md): what we build and why
- [Architecture](docs/design/CUSTODIAN_ARCHITECTURE.md): how it is built
- [Build plan](docs/design/CUSTODIAN_BUILD_PLAN.md): the ordered build steps
- [Roadmap to the real product](docs/ADVANCE.md): current status and step-by-step phases
- [Verification log](docs/VERIFY-LOG.md) and [component versions](docs/VERSIONS.md)
- [Current verified behavior and limits](docs/VERIFICATION-2026-10-16.md) and [completion requirements](docs/ACCEPTANCE.md)

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md). Report vulnerabilities privately as
described in [SECURITY.md](SECURITY.md).

## License

[Apache License 2.0](LICENSE)
