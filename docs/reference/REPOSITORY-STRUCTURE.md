# Repository structure and source-management policy

This repository contains a multi-runtime Windows desktop product. The layout
is intentionally organized by ownership rather than by historical worklog.

## Source ownership

| Directory      | Owns                                                                    | May contain generated files?                |
| -------------- | ----------------------------------------------------------------------- | ------------------------------------------- |
| `apps/desktop` | Tauri shell, React UI, native desktop integration                       | No; build output is ignored                 |
| `crates`       | Rust libraries, native authority, IPC, workers                          | No; `target/` is ignored                    |
| `services`     | Agent, broker, orchestrator, editor adapter, ML host                    | No; `dist/` and caches are ignored          |
| `proto`        | Shared schemas and generated-contract inputs                            | Only checked-in schemas                     |
| `tests`        | Cross-component, security, and end-to-end tests                         | No; caches are ignored                      |
| `scripts`      | Reproducible setup, fixture, verification, and build helpers            | No; outputs belong in temporary directories |
| `training`     | Synthetic datasets and model-evaluation tooling                         | Only small, reviewable source datasets      |
| `docs`         | Architecture, decisions, acceptance, and retained verification evidence | Only deliberate, reviewable evidence        |
| `video`        | Reviewable product-video scripts and captions                           | Rendered media is ignored                   |

Root files are workspace manifests, lockfiles, project entry-point documents,
build entry points and shared tool configuration. App, crate and service paths
remain stable so Cargo, npm workspaces, Tauri packaging and native adapters
continue to resolve the same source.

The documentation root contains only its index. `docs/design/` owns architecture
and decisions, `docs/plans/` owns proposed work, `docs/reference/` owns contracts
and policies, and `docs/verification/` owns dated execution records. The
`docs/EVIDENCE/` directory retains frozen inputs and cited supporting evidence.
Duplicate HTML exports and unreferenced raw logs are local archives, not source.

## Local working directories

| Directory       | Purpose                                                    | Versioned?       |
| --------------- | ---------------------------------------------------------- | ---------------- |
| `target/`       | Rust builds and temporary verification work                | No               |
| `models/`       | Downloaded model weights and local model inventory         | Placeholder only |
| `fixture/`      | Generated, marker-protected synthetic acceptance data      | Placeholder only |
| `deliverables/` | Rendered MP4, audio, previews and production intermediates | No               |
| `artifacts/`    | Ad-hoc logs, exports and evidence before review            | No               |
| `.local/`       | Machine-specific references and local archives             | No               |

The original large Cubby reference PDFs are preserved locally under
`.local/design-references/`. The application uses the versioned SVG/PNG artwork
under `apps/desktop/`; it does not depend on those PDFs. Existing rendered-video
paths under `deliverables/` are retained so local download links keep working.

## Never commit

- `target/`, `dist/`, `build/`, `coverage/`, package caches, Python caches, and
  compiler metadata.
- Downloaded models (`models/*` except the placeholder), credentials, `.env`
  files, local databases, runtime state, and `.freebuff/` identifiers.
- Generated acceptance fixtures under `fixture/`; create them with the fixture
  scripts in a temporary or explicitly local directory.
- Rendered video/audio exports. Keep the script, captions, and reproducible
  render instructions under `video/` instead.

## What belongs in a pull request

Every change should be attributable to one of the source directories above,
have a focused test or verification command, and keep generated evidence out of
the source diff unless it is intentionally retained as a dated audit artifact.
Do not mix local desktop runs, downloaded models, or build logs with product
source. Use `make test` for the offline test contract and `make check` for the
full formatting, lint, type, security, and test gate.

Run `npm run repo:check` before committing. CI runs this dependency-free check
before dependency installation is needed. It catches ignored files that are
already tracked, accidental root-level scripts, source files larger than the
project's 5 MiB review budget, missing npm workspace manifests, and missing
workspace manifests in the Docker dependency stage. This is a project policy,
not a universal size limit for all repositories. Larger distributable assets
belong in release/artifact storage, with source and provenance kept here.
It also rejects loose documentation files outside the index, unexpected docs
categories, and HTML exports in documentation folders.

Prettier, ESLint, mypy and the Docker build context exclude local output roots.
This prevents generated production helpers and downloaded dependencies from
being mistaken for maintained product source. Lockfiles, UI assets, source
fixtures, frozen acceptance inputs and deliberately retained audit evidence
stay versioned.

## Safe cleanup rule

Ignoring a path does not remove files already tracked by Git. Removing an
existing tracked fixture or audit artifact requires a separate review of every
consumer and should never be done by a blanket cleanup command.
