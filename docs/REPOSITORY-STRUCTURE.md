# Repository structure and source-management policy

This repository contains a multi-runtime Windows desktop product. The layout
is intentionally organized by ownership rather than by historical worklog.

## Source ownership

| Directory | Owns | May contain generated files? |
| --- | --- | --- |
| `apps/desktop` | Tauri shell, React UI, native desktop integration | No; build output is ignored |
| `crates` | Rust libraries, native authority, IPC, workers | No; `target/` is ignored |
| `services` | Agent, broker, orchestrator, editor adapter, ML host | No; `dist/` and caches are ignored |
| `proto` | Shared schemas and generated-contract inputs | Only checked-in schemas |
| `tests` | Cross-component, security, and end-to-end tests | No; caches are ignored |
| `scripts` | Reproducible setup, fixture, verification, and build helpers | No; outputs belong in temporary directories |
| `training` | Synthetic datasets and model-evaluation tooling | Only small, reviewable source datasets |
| `docs` | Architecture, decisions, acceptance, and retained verification evidence | Only deliberate, reviewable evidence |
| `video` | Reviewable product-video scripts and captions | Rendered media is ignored |

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

## Safe cleanup rule

Ignoring a path does not remove files already tracked by Git. Removing an
existing tracked fixture or audit artifact requires a separate review of every
consumer and should never be done by a blanket cleanup command.
