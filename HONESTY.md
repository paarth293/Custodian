# Honesty

This file lists what the code does **not** really do yet. If a module is a mock,
it must be listed here; `docs/VERIFY-LOG.md` records what was built, this records
what is real.

## Stubbed
Security-relevant mocks that **fail open or do nothing real** (must be replaced
before any of this guards real data):
- `custodian-engine::approval::ApprovalExecutor::verify_signature` accepts any
  non-empty signature. The real check lives in `skeleton::EngineSkeleton` and is
  not wired in. Nonces are kept in memory only, so they reset on restart.
- `custodian-core::install_lab_verify::verify_publisher` always returns `true`;
  `downloads::verify_download` only matches the string "eicar";
  `backup_check::check_backup_status` always returns `true`.
- `custodian-ipc::PeerMatrix::generate_key` derives keys from the role number
  (predictable). `services/mlhost/main.py` uses a hard-coded HMAC key.
- `custodian-winsec`: AppContainer spawn, job-object limits, Windows Hello and
  DPAPI are mocks. `protect_data` "encrypts" by reversing bytes, so quarantine
  vault contents are **not** encrypted and the vault key is the constant
  `mock_key_123`.
- `custodian-engine::ops::offload` overwrites the file with a marker and uploads
  nothing (destroys the content). `ops::online_only`, `ops::clone_dedupe` (plain
  copy) and `disk` do not call the Windows APIs they mention.
- `services/agent/src/model_loader.ts` "verifies" a model by checking it is not the
  string `corrupted`; the manifest hashes are `mock_hash` and are never compared.
- `apps/desktop/src-tauri`: `sign_approval`, `verify_hello`, `get_ledger` return
  canned values; the windowing/tray modules target the Tauri v1 API while
  `Cargo.toml` depends on Tauri v2. They are not in the Cargo workspace and are not
  built by CI.
- Everything else under `services/` (agent provider, search, voice, classifier,
  reader, session, broker actions/DLP/interception, orchestrator workflows) and
  `services/mlhost/src/*` is a mock returning canned values.
- `custodian-ledger::anchor` prints instead of persisting an anchor, so deleting the
  tail of the ledger is not detected.
- Placeholder tests, `#[ignore]`d with `todo!()` in
  `crates/custodian-engine/tests`: `gate_7`, `gate_9`, `gate_10`, `p2_extended`,
  `p9_p5_canary`, `p10_shadow_real`, `t_elev_install`. They verify nothing, so the
  "phase gate complete" entries in `docs/VERIFY-LOG.md` for gates 7-10 are
  claims, not results.
- Crates that are empty placeholders: `custodian-scan`, `-readapi`, `-parserhost`,
  `-kernel`, `-installlab`, `-evidence`, `-analyze`.

## Cut
(Nothing yet)

## Unmeasured
- The Rust suite has only been run on Linux. Windows-only tests (`cfg(windows)`) and
  the Windows-only link flags in `.cargo/config.toml` are exercised by CI alone.
- `custodian-store` SQL (DAOs, migrations, roles) is never run: CI has no Postgres.

## Not Entered
(Nothing yet)
