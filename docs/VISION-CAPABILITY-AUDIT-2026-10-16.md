# Verification against the desktop-operator vision — 16 October 2026

**Verdict: the current agent cannot execute the full supplied vision.** It is a
Windows assistant with a bounded set of real tools, rather than a general
desktop operator. Whole-drive discovery expands where certain file tasks work;
it does not add the ability to operate arbitrary apps or complete arbitrary tasks.

The supplied task examples are treated as a minimum sample of a much broader
product goal, not an exhaustive command list or a new limit on scope.

## Verification method

Reviewed the supplied vision, the model tool catalogue, Rust parsing and
execution routes, file and settings implementations, browser broker,
orchestrator, voice helpers, sandbox and elevation code. Checked implementations
and wiring rather than counting function names or trusting comments.

Fresh checks for this audit:

- Nine desktop tool-boundary tests passed, including unsupported-command
  refusal, allowed settings, argument validation and catalogue/desktop agreement.
- Eight actual fixture file-operation tests passed, including move/copy/rename/
  folder creation, reopening receipts, undo, changed-object refusal and tampering.
- A direct call to the compiled browser helper returned `Action click executed`
  for a nonexistent button with no browser. Its implementation validates a name
  and returns a string; it does not click anything.
- The compiled reader returned the same `summary: extracted` for two different
  input texts. That is a placeholder, not verified extraction or summarization.

Logs: `target/vision-audit-tools-2026-10-16.log`,
`target/vision-audit-file-tasks-2026-10-16.log`, and
`target/vision-audit-placeholder-probe-2026-10-16.json`.

Existing same-session evidence was inspected, not relabelled as a fresh run:
the final release's real desktop cross-drive search/approval/move/restart/undo,
C:/D: scan with independent answer and Stop, isolated AppContainer inference,
and model evaluations. See [the execution report](VERIFICATION-2026-10-16.md).
The 57-case model evaluations simulate desktop results. Their 98.25% scores do
not mean 98.25% of this vision is implemented.

No personal-file deletion, setting changes, software installations, account
messages, uploads, or transactions were used to audit missing capabilities.
Application implementation was not changed during this audit.

## Capability matrix

“Implemented, limited” means there is a real execution path for the stated
subset, not proof of every variation. “Missing” means there is no functioning
live agent path for the requested ability. “Unverified” identifies an acceptance
gap instead of treating untested behavior as working.

| Domain / requested ability | Current result | Evidence and boundary |
| --- | --- | --- |
| Desktop: open installed apps | Implemented, limited | Fixed Windows apps and Start-menu app lookup/launch; approval paths exist. Does not make the app's controls operable. |
| Desktop: list/close apps | Implemented, limited | Enumerates open windows and requests a normal close; no force-kill or general process manager. Broad live acceptance not established by this audit. |
| Desktop: click/type/navigate arbitrary apps | Missing | No general GUI/accessibility operator in the agent tool catalogue or execution routes. The external UI test script controls Custodian for testing; Custodian itself does not gain that ability. |
| Desktop: manage other windows/notifications | Missing as a general capability | Own Cubby placement and local reminders exist; arbitrary window layouts and notification management are not exposed execution capabilities. |
| Files: filename and largest-file search | Verified subset | Accessible local drives, explicit custom roots and measured metadata; real desktop C:/D: scope and cancellation tested. No network/cloud hydration or permission bypass. |
| Files: read contents / semantic search | Missing as a general agent capability | File-info tool reads metadata, not arbitrary document contents. Excerpt/parser and embedding paths are incomplete. |
| Files: create folders | Implemented and fixture-tested | Exact-path approval, receipt, verification and undo for supported local paths. |
| Files: move/copy/rename | Verified subset | Ordinary local files; actual cross-drive move and restart/undo tested. Directory-tree operations and universal filesystem support are not established. |
| Files: create/edit file contents | Missing | No general write/edit-file tool; composing chat text does not save or edit a requested document. |
| Files: organize | Partial | Individual file moves are possible. No general bulk organizing planner with verified multi-step execution. |
| Files: compress/archive | Missing | No live compress/extract workflow tool. |
| Files: back up/restore | Partial | Undo of supported operations exists. General backup jobs and backup-provider verification are not implemented. Undo is not a backup system. |
| Files: delete/clean disk | Implemented, restricted subset | Duplicate quarantine, old user-temp cleanup and controlled purge; no arbitrary delete-all tool. Quarantine may consume C: space for files from another drive. |
| Files: whole-PC scope consistently across tools | Partial | Search and explicit supported file mutations extend beyond personal folders. `open_folder` still accepts only known folders; `open_file_tool` still searches personal folders; cleanup keeps conservative scope. Whole-PC capability is not uniform. |
| Settings: appearance/file display/taskbar | Implemented, limited | Six named per-user settings with catalogued values, approval and undo. Existing registry evidence exists; live setting-card execution is not accepted end to end. |
| Settings: network/audio/display/Bluetooth/power/defaults/peripherals | Missing execution | Opening Windows Settings is not changing or verifying these settings. These controls are absent from the setting catalogue. |
| System: diagnose faults | Partial | CPU, memory, battery, uptime, disk facts and suggestions exist; no general diagnostic/repair workflow. |
| System: manage processes/startup apps | Missing as requested | App listing/normal close does not provide process/startup configuration. |
| Software: install/configure/update/uninstall | Missing live end-to-end capability | Install-lab/orchestrator/elevation helpers are incomplete or refuse unsupported actions; not connected to a verified chat workflow. |
| Development: approved commands/scripts | Missing | Model `run_command` is rejected. No restricted scripting executor is connected. Fixed internal helper commands are not a general execution tool. |
| Development: code/Git/build/debug | Missing as autonomous work | No agent tools to inspect/edit a codebase, invoke a build, use Git and verify results. The developer's ability to build Custodian is separate from Custodian's ability. |
| Web: launch site/search | Implemented, limited | Approval-controlled browser handoff. Opening a search does not mean the agent read or researched its results. |
| Web: research/read/navigate/click/type/forms | Missing / placeholder | `BrowserBroker.performAction` does not drive a browser; `readerExtract` does not extract the supplied material. |
| Web: upload/download/connected-account workflows | Missing live execution | No functioning browser/account workflow executor with verified outcomes. |
| Communication: email/messaging/calendar/meetings/contacts | Missing | No live tools or account adapters for these actions. |
| Communication: reminders/to-do | Implemented, limited | Local persisted to-do/reminder functionality; reminders depend on the app running or show when it next starts. No external calendar integration. |
| Productivity: Word/PDF/spreadsheets/slides | Missing artifact execution | No live editor/converter/generator tools; opening a document or drafting text in chat is insufficient. |
| Productivity: images/audio/video/media | Missing artifact execution | No connected generation/editing/conversion workflow. |
| Productivity: summarize/plan | Partial text ability | Local LLM can answer or draft text; arbitrary source ingestion and result verification are not implemented. Reader placeholder is not evidence of summarization. |
| Productivity: schedule/automate routines | Partial | Local reminders and fixed low-space/battery nudges; no general routine builder or scheduled cross-app execution. |
| Complex: coordinate unfamiliar multi-app/API/web work | Missing as a general operator | Some task lifecycle/concurrency machinery exists. Orchestrator workflows/IPC contain log-only placeholders and no general executable plan/observe/replan loop over UI/browser/script tools. |
| Platform: Windows | Partial product | Native build, installer, limited live execution and sandbox inference verified. Clean-profile install/uninstall and full acceptance remain open. |
| Platform: macOS/Linux | Not delivered/verified | Portable types and fallback branches are not functioning platform operators; no corresponding live execution acceptance. |

## User expectations

| Requirement | Result |
| --- | --- |
| Natural text interaction | Real local-model text chat plus deterministic routes. Most tool routing is English-oriented; robust multilingual execution and accessibility acceptance are unverified. |
| Optional voice | Missing: STT/TTS helpers return fixed/empty values. |
| Understand → plan → execute → verify → summarize | Demonstrated for selected file tasks; not available for arbitrary tasks or unfamiliar workflows. |
| Never claim success without checking | Not satisfied universally: browser placeholder claims execution; reader returns a fixed summary; orchestration IPC returns success after logging. These are not validated completion signals. |
| Show work / Stop / pause | Task panel and scan Stop verified for a live subset. No general pause/resume or instant interruption of synchronous OS operations. |
| Approval and safe recovery | Real controls for selected actions; file restart/undo tested. General race hardening and abrupt mutation-time crash recovery remain open. |
| Preferences / per-app permissions / routines | Some own-app settings and fixed nudges exist. User-configurable per-app authority and general routines are not implemented. |
| Local-first deployment | Real offline pinned-model path and AppContainer. Broader integrations, predictable latency, clean-profile installation and cross-platform recovery remain unverified or missing. |
| Product success metrics | A narrow model-case gate and fixture/live evidence exist. No representative benchmark of unfamiliar multi-app task success, privacy enforcement, time saved and rollback reliability across the vision. |

## Security requirements

**The current security model does not satisfy the full supplied specification.**
The model is constrained, but the application has no universal policy mechanism
enforcing the new vision before every read, write, GUI interaction, command and
network transfer.

| Required boundary | Current evidence / gap |
| --- | --- |
| Independent policy before every operation | Rust validates supported tool calls and controls approvals; model inference is sandboxed. These are useful controls, but no universal scoped policy/consent engine covers all desktop helpers, reads and future channels. Engine approval code is not uniformly wired into the live desktop. |
| User “Never Access” folders/files/apps/accounts/screens | Missing. No user-defined private-zone enforcement is passed into discovery, metadata inspection, context logging or future screen/account access. |
| Secrets restricted by default | Protected-location checks block some mutations, including credential locations. Whole-system read discovery has no equivalent secret/private-zone policy. Mutation denial does not prevent metadata disclosure; the AppContainer does not constrain the trusted desktop's tool reads. No real credentials were read to test this. |
| Scoped temporary permissions; no always-on admin | Default model AppContainer and user-level operations are real. No complete per-task/per-app grant and revocation system; elevation verbs are unsupported rather than a working approved elevation workflow. |
| Risk tiers and consequential previews | Some cards, known setting values, protected paths and cleanup confirmations exist. No common risk classifier/authorization contract for the whole requested capability surface. |
| Critical actions blocked / independent administrative procedure | Unknown commands and protected file changes are refused in supported routes. A separate administrative procedure for exceptional critical operations is not implemented. |
| Prompt/plugin/command/path/exfiltration resistance | Selected argument/path tests and sandbox controls exist. Reader injection detection is two text markers, not broad resistance. No integrated evidence for private-zone bypass, malicious cross-app content, plugin trust and destination-bound network permission across all channels. |
| Contained scripts/extensions and approved destinations | No working restricted script/extension executor; browser/network broker integration incomplete. Absence of a capability is not proof of a finished safe implementation. |
| Rules/consent/log tamper resistance | Model cannot request arbitrary tool names; selected receipts/anchors use DPAPI and approvals use signing mechanisms. Same-user tampering/automation and incomplete trusted-click separation remain limitations. No signed release/update chain; current installer is unsigned. |
| Audit/emergency Stop/encrypted secrets | Selected sealed records and live scan Stop exist. These do not establish universal instant Stop, protected audit storage or complete account-secret management. |
| Continuous adversarial testing | Targeted regressions exist, but no comprehensive continuous red-team acceptance covering the supplied security list. Four deletion-claim evaluation cases are not a general security benchmark. |

Broadening reads without private-zone enforcement is a gap against this vision.
The next capability expansion should follow the user's stated build order:
secure permission boundaries and truthful outcomes first, then broad app/file
primitives, real browser/cross-app execution, automation, and hardened platform
releases. This is a verification finding, not a claim those additions were built.

## Source anchors

- Catalogue and closed dispatch: `services/agent/src/tools.ts`,
  `apps/desktop/src-tauri/src/assistant/tools.rs`,
  `apps/desktop/src-tauri/src/assistant/mod.rs`.
- Six settings: `crates/custodian-winsec/src/settings.rs`.
- Supported verified file mutations: `crates/custodian-core/src/file_tasks.rs`.
- Read discovery without private-zone input: `crates/custodian-core/src/walk.rs`.
- Browser success placeholder: `services/broker/src/index.ts`.
- Fixed extraction: `services/agent/src/reader.ts`.
- Logging-only IPC/workflows: `services/orchestrator/src/activities.ts` and
  `services/orchestrator/src/workflows/`.
- Voice placeholders: `services/mlhost/mlhost/voice.py`.
- Unimplemented privileged actions: `crates/custodian-elevate/src/main.rs`.
- Deployment and unresolved findings: [HONESTY.md](../HONESTY.md),
  [acceptance checklist](ACCEPTANCE.md),
  [audit inventory](FIX-INVENTORY-2026-10-15.md).

The evidence supports a working, limited Windows assistant. It does not support
claiming that the broad vision, universal privacy boundary or general task
execution has been completed.
