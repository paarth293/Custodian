# Honesty

What the code does today versus what the design promises. Update this file in
the same commit that stubs, cuts or completes something.

## Stubbed

Placeholders with their final signatures but without the real implementation.

### Windows integration (`custodian-winsec`, `custodian-core`, desktop shell)

- `spawn_in_appcontainer` starts a normal child process; no AppContainer.
- `apply_job_limits` does not create a job object.
- `prompt_biometrics` and the desktop `verify_hello` always succeed (no Windows Hello).
- `protect_data` / `unprotect_data` reverse bytes instead of calling DPAPI:
  **no confidentiality**. Quarantine therefore does not encrypt yet.
- `check_backup_status`, `detect_sandbox` and `verify_publisher` always return `true`.
- `verify_download` only flags files named like the EICAR test file.
- Install-lab `parse_diff`, `record_recipe`, `undo_install` and `prepare_uninstall` do nothing.
- `OnlineStateMachine` does not terminate the broker job.
- The input hook that would call `record_real_click` is not installed.

### Engine (`custodian-engine`)

- `ApprovalExecutor` checks only that a signature is present; full Ed25519
  verification exists in `EngineSkeleton::verify_approval` but is not wired in.
- `OffloadOp` replaces the file with a stub but uploads nothing.
- `OnlineOnlyOp` is a no-op (no `CfSetPinState`).
- `CloneDedupeOp` copies instead of block-cloning.
- `AnchorManager` persists nothing.

### Sandbox and parser

- `custodian-sandbox` `build_argv` returns a fixed argv; `capsprobe` only prints.
- `custodian-elevate` validates verbs but performs no privileged action.
- `ExcerptPipeline` does not invoke `custodian-parser`.

### Services

- Agent: `server.ts` answers chat over authenticated stdio frames with a local
  GGUF model (`node-llama-cpp`). There is still no classifier, search or tool
  execution in the agent; the desktop runs all tools itself. `readerExtract`
  flags prompt-injection markers but returns a fixed summary instead of
  extracting data. The offline planner prompt is a one-line placeholder.
- Broker: actions are validated but no browser is driven.
- Orchestrator: workflows log and return; activities do not send IPC.
- ML host: `extract_embeddings` returns a zero vector; `safety`, `tabpfn_models`
  and `voice` return fixed values. `decide` runs the real model when it is
  present in `models/`, otherwise abstains.
- Desktop: approval signing (`approval_service::sign_approval`) and the
  approval window are Rust-only by design but not yet called by an engine
  dispatcher; the signature is a tagged string, not a real signature.
- Desktop chat: the commands Cubby understands (duplicates, biggest files, free
  space, safe clean-up, undo, history, opening apps, folders and websites, web
  search, time and date) run for real. Clean-ups use the Rust quarantine in
  `custodian-core` with hash checks and undo, not the journaled
  `custodian-engine`, and they do not write to `custodian-ledger`.
  `approve_plan` has no signature or genuine-click check: any script in the
  webview could approve a pending plan.
- Desktop chat model: open-ended messages need a GGUF chat model in `models/`
  (or `CUSTODIAN_CHAT_MODEL`). It is not checked against `models/manifest.json`
  yet, and no model has been run in CI. The agent and Node.js are not bundled
  into the installer, so the chat model works only from a repository checkout
  after `npm run build`.
- Mode colour: Online mode is never reported because the broker process does
  not exist yet, so the amber Special Form appears only as a labelled preview
  (tray or right-click menu).
- `tools/fetch.ps1` writes a mock `tools/tools.toml` instead of fetching and
  pinning the sandbox tools.

## Cut

(Nothing yet)

## Unmeasured

- Performance budgets (idle CPU, memory, startup time) have not been measured.
- Laya's accuracy has not been evaluated on held-out data.

## Not Entered

(Nothing yet)
