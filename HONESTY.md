# Honesty

What the code does today versus what the design promises. Update this file in
the same commit that stubs, cuts or completes something.

Completion is governed by [the acceptance checklist](docs/reference/ACCEPTANCE.md).
The [19 October verification audit](docs/verification/VERIFICATION-AUDIT-2026-10-19.md)
records fresh checks of the current checkout, remaining stage gates and the
installed Excel version mismatch. Passing core tests does not establish that
all stages or desktop routes are complete.
The current [capability register](docs/reference/CAPABILITY-REGISTER.md) and
[Stage 0 baseline audit](docs/verification/STAGE-0-BASELINE-2026-10-17.md) reconcile current
routes and evidence. Older sections below can describe paths now paused or
unavailable; the register distinguishes these from verified subsets and gaps.
The [Stage 0 runtime repair](docs/verification/STAGE-0-RUNTIME-REPAIR-2026-10-17.md) revalidated
the existing baseline after correcting agent scheduling. It adds no broader
operator capability.
The [Stage 1 authority/recovery increment](docs/verification/STAGE-1-AUTHORITY-AND-RECOVERY-2026-10-18.md)
passes its declared native, privacy and recovery cases and a 27-check optimized
desktop run. Optional standing-grant scopes retain explicitly requested local
reach when empty. Screen/app observation stays disabled; general app control,
full security acceptance, compiler hardening and installation remain open. Its
native executable includes these changes; the historical Stage 0 installer does
not. Laya remains inactive.
The [Stage 2 coordinator increment](docs/verification/STAGE-2-TASK-COORDINATOR-2026-10-18.md)
adds bounded native composition and step evidence to real file collections.
The app adapter in composition fixtures is explicitly mock. Pause is at program
boundaries; restart requires review and never replays a saved program. Most
legacy routes still use their original lifecycle, and general model planning
and real app automation remain unfinished.
The [Stage 3 language/planning increment](docs/verification/STAGE-3-LANGUAGE-AND-PLANNING-2026-10-18.md)
adds Qwen-owned collection selection and source/destination interpretation with
native location bindings and capability compilation. All 24 supported frozen
interpretations executed real fixture copies and undo; all 12 negative cases
were refused. This is a small authored regression, not general language or
security acceptance. Unsupported exclusions, ambiguous plural references and
non-ASCII goals are clarified. Laya remains inactive after historical paired
advisory replay; no new live safeguard is claimed. The report tracks exact
desktop/artifact acceptance separately.
Historical phase entries based on simulations are not product acceptance.
The [Stage 4 foundation review](docs/verification/STAGE-4-FOUNDATION-REVIEW-2026-10-18.md)
distinguishes typed app contracts/native admission tests from real app execution.
Process probes read executable/creation metadata only. Private-app policy is a
native library boundary, not a persisted user setting or qualified UIA adapter.
The subsequent [supervised worker increment](docs/verification/STAGE-4-SUPERVISED-WORKER-2026-10-18.md)
adds process-enforced deadlines, authenticated bounded IPC and worker-only cleanup.
This trusted normal-user worker is resource-contained, not an AppContainer or
filesystem/network sandbox. Its production operations probe its own identity
and empty message-only window; all real app operations return unavailable.
Durable app-journal integration, persisted privacy controls and qualified real
application adapters remain unfinished. The worker is not wired into live chat
or packaged into a new desktop/installer by that increment. The subsequent
[persistent app privacy increment](docs/verification/STAGE-4-APP-PRIVACY-2026-10-18.md) adds
encrypted native app exclusions and a desktop executable exclusion list.
Native window/session/document exclusions share the policy; their real-world
binding and user selection remain part of adapter qualification. General
content observation stays disabled. These rules do not disable the excluded
application or apply its exclusion to file tools; private file locations remain
a separate control. Those reports describe historical increments. The subsequent
[real application implementation](docs/verification/STAGE-4-REAL-APP-IMPLEMENTATION-2026-10-18.md)
adds actual version-qualified VS Code and Excel workflows, native desktop
approval, independent output verification, publication undo and encrypted app
recovery inspection. General window reading and screen capture remain disabled.
Stage 4 acceptance is separate from implementation and requires the repeated
packaged checks and disposable offline installation evidence in that report.
The [Stage 5 execution record](docs/verification/STAGE-5-EXECUTION-RECORD-2026-10-18.md)
adds exported, offline-tested core primitives for exact batch manifests,
bounded ZIP round trips, immutable versioned backup/restore and separately
authorized governed recycle. These are not yet desktop language/planner routes;
the Windows recovery provider is intentionally unavailable, so no recycle or
Stage 5 whole-system acceptance is claimed. Same-machine backups are not
ransomware-proof and unsupported metadata, reparse points, cloud placeholders,
hardlinks and network volumes remain refused.
The [Stage 6 execution record](docs/verification/STAGE-6-EXECUTION-RECORD-2026-10-18.md)
adds a bounded native TXT/Markdown/CSV reader and staged structured verifier
with source hashes, identities, locations, warnings and explicit partial
results. It is a core subset only: it is not connected to the chat planner or
contained parser worker, and no DOCX/PDF/PPTX/image/OCR/audio/video support is
claimed. The existing parser binary and web-marker reader remain unavailable
content routes.
The [offline foundation implementation](docs/verification/OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md)
supersedes earlier live-capability claims for the current source. File privacy
controls and offline refusals are implemented; legacy cleanup/purge and broad
app actions are intentionally paused pending migration. The full operator and
all privacy/security gates remain incomplete.
The [17 October routine backup increment](docs/verification/ROUTINE-BACKUP-IMPLEMENTATION-2026-10-17.md)
adds an opt-in native largest-file copy workflow with standing permission,
size/capacity limits, Windows copy guards and durable undo. It does not enable
general autonomous model execution; its final desktop evidence is recorded in
that report.
The [model-planned workflow increment](docs/verification/MODEL-PLANNED-WORKFLOWS-2026-10-17.md)
adds a shared bounded collection engine, separate opt-in permission, Qwen
capability proposals with native validation, sealed step tracking, restart
inspection and group undo. Planning is constrained by a native selection
contract and is not general open-ended automation. Model experiments and final
desktop evidence are recorded separately in that report.
The [17 October typed Laya prototype](docs/verification/LAYA-VALIDATION-IMPLEMENTATION-2026-10-17.md)
now performs actual local typed-model inference in a separate development
evaluator. Its first measured candidate fails activation targets and remains
disconnected from desktop authority; it is not an active production safeguard.
The [follow-up experiments](docs/verification/LAYA-IMPROVEMENT-VERIFICATION-2026-10-17.md)
add structured effect checks and faster selective/encoder-INT8 inference, but
both tested model judges still fail semantic reliability requirements.
The [16 October verification](docs/verification/VERIFICATION-2026-10-16.md) records whole-PC
scope, actual cross-drive desktop execution/restart/undo, model scores and
remaining acceptance gaps.

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
- If the agent cannot load its model inside the container, startup fails
  closed and deterministic desktop tools remain available. There is no
  automatic unrestricted fallback. `CUSTODIAN_AGENT_SANDBOX=0` or
  `CUSTODIAN_AGENT_UNRESTRICTED=1` explicitly enables development-only
  unrestricted startup; that path still starts the process before its job.
- The older `spawn_in_appcontainer` in `custodian-winsec` (used by
  `sandbox_factory`, not by the agent) still starts a normal child process.
- Safe places have no owner-only DACL and no reparse-point check yet. Quarantined
  files are moved, not encrypted (the design's zero-copy rename); only sidecars,
  keys and the ledger anchor are sealed with DPAPI.
- Backup and publisher checks report unavailable until their Windows providers
  are connected. Sandbox detection checks for WindowsSandbox.exe without a
  launch/feature-readiness probe. Download verification fails closed
  when a scanner is unavailable; EICAR detection is still filename-based.
- Install-lab `parse_diff` and `record_recipe` remain placeholders.
  `undo_install` and `prepare_uninstall` refuse unsupported operations.
- `OnlineStateMachine` does not terminate the broker job.
- The input hook that would call `record_real_click` is not installed.

### Settings (`custodian-winsec::settings`, desktop chat)

Changing Windows settings is real, not planned. Six per-user settings can be
read, changed and put back: dark mode for apps, dark mode for Windows,
transparency effects, hidden files, file name extensions and taskbar
alignment. A change is shown on a card first **even when the person asked for
it themselves**, is written to `HKEY_CURRENT_USER` only after that click, and
records the value it replaced in a DPAPI-sealed file and in the ledger, so
"put my theme back" works after a restart. Both halves of a change (the
setting and the value) come from the catalogue, so the language model cannot
name anything outside it. What is not covered yet:

- Only these six settings, and only per-user ones. No machine-wide settings,
  nothing that needs administrator rights or elevation, and no sound,
  brightness, Wi-Fi, Bluetooth, power plan, default apps or startup apps.
- Value words are a fixed vocabulary ("on", "off", "light", "dark", "left",
  "center", "show", "hide", ...). A phrasing outside it gets a question
  rather than a guess, and a request matching two settings equally is always
  asked about.
- "Put the last setting back" undoes the most recent change only. There is no
  list of past changes in the panel, and "undo" still means undoing a file
  clean-up.
- Nothing creates a Windows restore point, and the card describes the effect
  in words instead of previewing it.
- Covered by tests down to the registry (`cargo run -p custodian-winsec
--example settings -- <setting> <value>` changes a setting, reads it back
  and puts it back). Clicking the card in the running app has not been
  exercised end to end on a real desktop.

### Engine (`custodian-engine`)

- `OffloadOp` and `OnlineOnlyOp` return Unsupported without modifying files
  until durable remote upload/rollback and `CfSetPinState` are implemented.
- `CloneDedupeOp` copies instead of block-cloning.
- The ledger anchor is sealed with DPAPI, not signed with a TPM key, so code
  running as the same user could rewrite both the ledger and its anchor.
- The Engine is not an IPC process yet: `ApprovalExecutor` and `QuarantineOp`
  are real but only exercised by tests; no plan reaches them from the UI.

### Sandbox and parser

- `custodian-sandbox` expands configured arguments; `capsprobe` only prints.
- `custodian-elevate` validates verbs and refuses unsupported privileged actions.
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
  list, privacy status, time and date, and changing Windows settings after a
  click on a card. Tool calls from the model are checked
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
  agent because it hid the GeForce and stalled the GPU check. A GPU backend
  that does not answer its first query within 30 s (`GPU_CHECK_MS`) is
  abandoned and the CPU is used instead, so a broken driver cannot hold a
  reply up indefinitely. That cap is not optional: at 5 s it timed out on
  every start on the development laptop and every reply ran on the CPU —
  about 150 s instead of about 3 s (measured 2026-10-12). No model has been run in CI.
  The packaging step now stages Node, the agent, runtime dependencies and a
  SHA-256-verified CPU model. Clean-profile installation still needs acceptance.
- Mode colour: Online mode is never reported because the broker process does
  not exist yet, so the amber Special Form appears only as a labelled preview
  (tray or right-click menu).
- Verified external sandbox-tool fetching remains unimplemented. The former
  `tools/fetch.ps1` only wrote a mock manifest and has been removed. No working
  tool capability was removed. External tools still require reviewed sources,
  cryptographic pins, containment and independent output verification.

### Background tasks and file operations

- The task coordinator saves sealed lifecycle history. On reopening, completed
  tasks remain completed and interrupted work requires recovery; changes are not
  replayed automatically. It checks overlapping paths and retains executing
  workers' resource locks until they acknowledge cancellation.
- File move, copy, rename and folder creation save sealed prepared receipts
  before mutation and verify results before recording completion. Verified undo
  works after reopening the records. Approval cards now capture absolute paths,
  source identity/content and destination-parent identity; changed objects and
  occupied destinations are refused. Fixture tests also cover resolving “it”
  against measured search results, ambiguity and stale conversation results.
  This does not close path-race hardening or abrupt process-death recovery.
  Cancellation cannot interrupt a synchronous operating-system call mid-call.
- The active-task panel and fresh-conversation control need interactive
  desktop acceptance testing. Automated tests do not establish that every
  design goal is implemented.

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

### Tool accuracy (2026-10-12, Qwen3 4B on Vulkan, 57 evaluation cases)

Measured with `node services/agent/eval/run.mjs <model>` on the development
PC, with the desktop's tools stubbed to realistic results (so the model alone
is measured):

- Whole suite: 49/57 (86%) passed, against 43/56 (77%) before the settings
  work. Right tool 86% (was 79%), right arguments 86% (was 79%), right answer
  98%, safe wording 100%. Still below the Phase 6 gate of 95% tool selection,
  which is the honest current score.
- The settings cases went from 1/7 to 6/7. Two real defects were fixed on the
  way: the planner prompt told the model it _cannot change anything_ (so it
  refused, saying it had no permission), and it described approval as
  something to ask for in words, so the model asked "shall I?" and stopped
  instead of calling the tool that shows the card. The `change_setting`
  description now names every setting, so no look-up round trip is needed.
- The one remaining settings-adjacent miss is "I need to change my display
  settings": display resolution and scaling are not in the catalogue, and the
  model asks what to adjust rather than opening the Settings app as the
  evaluation expects. The expectation is left as it stands rather than
  rewritten to match the model.
- Per message on the GPU: first word 0.3-0.7 s, whole reply 2.3-3.3 s, model
  load 16 s, 3.1-3.2 GB held while loaded.

## Unmeasured

- Frame rates while animating and sleeping (D-26) have not been measured.
- Laya's accuracy has not been evaluated on held-out data.
- The settings card has not been clicked in the running app; the registry
  write, read-back and revert are covered by `cargo run -p custodian-winsec
--example settings -- <setting> <value>`, which changes a real setting and
  puts it back.
- How fast the model loads and answers inside the AppContainer: the first
  run was measured while another model was running, so its time is not a
  fair number.

## Not Entered

(Nothing yet)

## Verification on 2026-10-15

- Automated workspace checks pass: Rust formatting, Clippy with warnings denied,
  296 Rust tests (three opt-in tests excluded from the routine run), JavaScript
  build/typecheck/lint/formatting and 92 tests, Python lint/format/typecheck and
  21 tests including authenticated stdio smoke tests. Python ran on the locally
  installed 3.11.9, rather than the CI target of 3.12.
- Explicit C: to D: temporary-file tests verify moves, checks and rollback.
- Full Qwen3 4B evaluation with simulated desktop results: baseline 49/57 (86%),
  revised routing prompt 52/57 (91%) overall,
  57/57 safety checks; the 95% acceptance gate fails. This measures the model
  alone, not the rule-based desktop routes or real execution.
- The real model answers under job limits. The first sandboxed CPU integration
  attempt and isolated CPU retry timed out during tool-prompt preparation.
  Explicit Vulkan initialization stalled. Sandboxed chat is not acceptance-ready.
- Rust dependency policy checks and Python dependency audit pass. Production
  JavaScript dependencies have zero reported vulnerabilities. The full JavaScript
  audit reports two moderate issues through development-only quicktype/stream-json,
  with no available fix. No dependencies were force-upgraded.

The native production executable and Windows NSIS installer built successfully.
The current system/tool prompt measures 3,319 tokens for both pinned Qwen models;
the minimum context is now 4,096 instead of 3,072 so the tool instructions fit.
See [the current verification report](docs/verification/VERIFICATION-2026-10-15.md) for details.
