# Honesty

What the code does today versus what the design promises. Update this file in
the same commit that stubs, cuts or completes something.

## Stubbed

Placeholders with their final signatures but without the real implementation.

### Windows integration (`custodian-winsec`, `custodian-core`, desktop shell)

- The chat agent runs in a real AppContainer (`custodian-winsec::appcontainer`):
  no capabilities, so no network (tested: `curl` fails inside it), and read
  access only to Node's folder, the agent package, the `node_modules` holding
  its runtime and `models/` (tested: a file outside those cannot be read, and
  nothing can be written even where reading is allowed). It starts suspended
  and is placed in its job object before it runs. The grants add one
  inheritable read entry for the container to those folders' permissions;
  the first time takes about 15 s for `node_modules`.
- If the agent cannot load its model inside the container (its `ping` fails),
  it starts as a normal process instead, the reason is logged, and "is my
  data safe?" says so. The normal path still starts the process before it is
  in its job. `CUSTODIAN_AGENT_SANDBOX=0` turns the container off.
- The older `spawn_in_appcontainer` in `custodian-winsec` (used by
  `sandbox_factory`, not by the agent) still starts a normal child process.
- Safe places have no owner-only DACL and no reparse-point check yet. Quarantined
  files are moved, not encrypted (the design's zero-copy rename); only sidecars,
  keys and the ledger anchor are sealed with DPAPI.
- `check_backup_status`, `detect_sandbox` and `verify_publisher` always return `true`.
- `verify_download` only flags files named like the EICAR test file.
- Install-lab `parse_diff`, `record_recipe`, `undo_install` and `prepare_uninstall` do nothing.
- `OnlineStateMachine` does not terminate the broker job.
- The input hook that would call `record_real_click` is not installed.

### Engine (`custodian-engine`)

- `OffloadOp` replaces the file with a stub but uploads nothing.
- `OnlineOnlyOp` is a no-op (no `CfSetPinState`).
- `CloneDedupeOp` copies instead of block-cloning.
- The ledger anchor is sealed with DPAPI, not signed with a TPM key, so code
  running as the same user could rewrite both the ledger and its anchor.
- The Engine is not an IPC process yet: `ApprovalExecutor` and `QuarantineOp`
  are real but only exercised by tests; no plan reaches them from the UI.

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
- Desktop: the separate approval window (`approval_window`) is not used;
  approvals are confirmed in a native dialog instead (see below).
- Desktop chat: everything Cubby understands runs for real, by rules first
  and by the local model's tool calls otherwise: finding files by name, disk
  space, biggest files, duplicates, system status, suggestions, temp files,
  undo, history, opening Start-menu apps, documents, folders and websites,
  web search in the browser, closing apps, open apps, reminders and the to-do
  list, privacy status, time and date. Tool calls from the model are checked
  again in Rust (known folders, allow-listed apps, short printable text) and
  anything that sends data off the PC or changes things (web search, a
  website, undo, closing an app, opening a file or an app) waits for a click
  on a card. Results shown to the person are written by Rust, not the model.
- Clean-ups use the Rust quarantine in `custodian-core` with hash checks and
  undo, not the journaled `custodian-engine`. Copies stay in the safe place
  for 30 days; the space frees only when the window ends (or "empty the safe
  place", confirmed with Windows Hello) and only after the copy and the kept
  original are both re-checked; anything that does not match stays. Undo puts
  files beside a new file rather than over it, can be retried after a
  failure, and re-creates purged copies from their original. The safe place
  is in `%LOCALAPPDATA%`, so copies from another drive use space on C: until
  purged, and purges run only while Custodian is running.
- Copies inside OneDrive (and other folders a sync client registers) are
  never proposed; when a group has one copy there, that copy is the one kept.
- Temporary files: only regular files in the person's own `%TEMP%`, untouched
  for a week, re-checked just before deletion; they are deleted for good
  (said on the plan card), after the same signed approval. Temp clean-ups are
  in the ledger but not listed by "what did you do?".
- Each clean-up, temp clean-up, purge and undo is recorded in
  `custodian-ledger`, checked against its anchor first. `approve_plan` asks
  with Windows Hello when it is set up (a system prompt no other program can
  answer) or a native dialog otherwise, signs the plan digest with a
  DPAPI-sealed Ed25519 key, and the operation verifies signature, expiry and
  nonce before changing anything. Without Hello, other programs running as
  the user could answer the dialog (the hardware-click hook is not installed).
- Reminders and the to-do list live in a DPAPI-sealed file; reminders fire
  only while Custodian is running (missed ones show at the next start).
- Closing apps sends the same request as the X button; apps running as
  administrator cannot be asked, and nothing is ever force-quit.
- Unprompted nudges are limited to a drive with under 5 GB free (once a day)
  and a battery at 15% or less (once an hour), never while Windows reports
  the person busy (fullscreen, presenting) or away.
- Cubby's window is excluded from screen capture; the tray can allow it for
  an hour.
- Desktop chat model: open-ended messages need a GGUF chat model in `models/`
  (or `CUSTODIAN_CHAT_MODEL`), pinned in `models/manifest.json`; its SHA-256 is
  checked before it loads. It runs on the first GPU backend that works and on
  the CPU otherwise (`CUSTODIAN_LLAMA_GPU=off` forces the CPU). CUDA needs
  NVIDIA's CUDA runtime, which the development PC lacks, so it uses Vulkan.
  Without a GPU, a default model over 1.5 GB gives way to the smallest pinned
  chat model. AMD's switchable-graphics Vulkan layer is turned off for the
  agent because it hid the GeForce and stalled the GPU check; a GPU check that
  hangs for another reason holds up the first answer until node-llama-cpp's
  own 5-minute limit. No model has been run in CI. The agent and Node.js are
  not bundled into the installer, so the chat model works only from a
  repository checkout after `npm run build`.
- Mode colour: Online mode is never reported because the broker process does
  not exist yet, so the amber Special Form appears only as a labelled preview
  (tray or right-click menu).
- `tools/fetch.ps1` writes a mock `tools/tools.toml` instead of fetching and
  pinning the sandbox tools.

## Cut

(Nothing yet)

## Measured

Release build, Ryzen 5 3550H, 8 logical cores, 14 GB RAM, 2026-10-05:

- Desktop app (D-26): window visible 0.62 s after launch; idle CPU 0.04%
  (budget < 1%); 180 MB private memory for the whole process tree, including
  WebView2 (budget 600 MB with models unloaded).
- Chat (Qwen 2.5 1.5B Q4_K_M, CPU): the model loads in the background in
  about 8 s when the chat panel opens; then the first word of a reply arrives
  in about 0.6 s and a short reply completes in about 2 s. The agent uses
  about 1.8 GB while loaded, under a 4 GB job limit, and is unloaded after
  10 minutes without a question.

## Unmeasured

- Frame rates while animating and sleeping (D-26) have not been measured.
- Laya's accuracy has not been evaluated on held-out data.
- How fast the model loads and answers inside the AppContainer: the first
  run was measured while another model was running, so its time is not a
  fair number.

## Not Entered

(Nothing yet)
