# Fix inventory — 15 October 2026

This is the complete 46-finding inventory from the existing audit, plus the
live verification defects below. A mitigation that disables an unsupported
operation is not the implementation of that feature. "Open" and "Partially
corrected" are intentionally not claimed as fixed.

## Live verification defects being addressed

| ID | Defect | Required evidence | Status |
| --- | --- | --- | --- |
| V01 | CPU model preloads every tool and misses its response deadline | Real sandboxed answer under the unchanged deadline | Focused sandbox CPU knowledge/tool smoke passes; broad latency acceptance remains open |
| V02 | Five requests still miss the intended tool | Unchanged 57-case evaluation at >=95% | CPU and Vulkan each 56/57 (98.25%), gate passed; one miss each retained |
| V03 | Mutating tool proposals can be narrated as completed changes | Pending/failed tool result regressions | Deterministic action narration and pending/failed result tests pass |
| V04 | Cancellation can leave a tool-result wait alive for ten minutes | Cancellation resolves the wait promptly and sends no further tool calls | Focused cancellation and no-late-call tests pass |
| V05 | Concurrent chats clear each other's activity state; a single plan slot overwrites older plans | Overlapping-message and multiple-plan UI tests | State regression tests pass; live evidence is tracked in the current report |
| V06 | Task state is lost after a restart | Reopen stored task state and report interrupted tasks honestly | Persisted reopen/recovery-required tests pass; abrupt live crash acceptance remains open |
| V07 | Move/copy/rename/create-folder lack durable verified inverse records | Fixture execute/restart/undo and changed-target refusal | Backend fixtures and real C:-to-D: move/restart/undo pass; general race/crash gaps remain |
| V08 | Installer lacks agent and Node runtime | Staged installed runtime loads without the source checkout | Runtime staged and isolated AppContainer inference passes; clean-profile installation open |
| V09 | quicktype CLI introduces vulnerable development dependencies | Preserve schema generation and clean full dependency audit | Schema generation passes; full audit reports zero known vulnerabilities |
| V10 | Clean-profile installer and interactive desktop acceptance unverified | Isolated installation/uninstallation and live UI evidence | Real desktop cross-drive journey passes; clean-profile install/uninstall and repeated walkthroughs remain open |

Evidence and qualifications: [16 October verification](VERIFICATION-2026-10-16.md).

## Audit findings

| Finding | Required correction | Current status | Closure evidence or remaining work |
| --- | --- | --- | --- |
| F01 | The AI sandbox is opt-in and falls back to an unrestricted process | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F02 | Resource limits fail open and normal startup has a race | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F03 | Desktop cleanup recovery entries are persisted after mutations | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F04 | Cleanup manifests are unauthenticated authority-bearing input | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F05 | Ordinary rename undo does not verify payload content | Verified mitigation | Hash verification and rollback tests pass. |
| F06 | Path validation and content checks are separated from the operation | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F07 | Purge treats cloud size and timestamp as proof of identical content | Verified mitigation | Cloud placeholders cannot authorize purging. |
| F08 | Concurrent cleanup and recovery operations have no common lock | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F09 | Offload destroys content despite being a placeholder | Verified mitigation | Unsupported offload leaves the original untouched. |
| F10 | Other public mutation APIs bypass the intended execution contract | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F11 | Integrity checks run after changes and missing history can appear healthy | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F12 | Security guarantees exceed the same-user protection boundary | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F13 | Offline policy is not centrally enforced for network handoffs | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F14 | Stop can wait indefinitely for the next agent frame | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F15 | Cancellation is global and long file work is not fully interruptible | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F16 | Model action claims are checked only when no tool was called | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F17 | File discovery does not deliver the requested file-detail experience | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F18 | Search scope can discard the broader root | Verified mitigation | Parent search roots are retained. |
| F19 | Scan incompleteness is not fully represented | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F20 | Repeated scans and duplicate progress undermine optimization | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F21 | Same-volume and cross-volume undo do not preserve equivalent metadata | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F22 | Quarantine can worsen low space on the system drive | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F23 | Settings are changed before undo persistence and without read-back | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F24 | Settings undo can overwrite newer user choices or lose exact values | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F25 | Settings coverage and conversational follow-through are narrow | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F26 | Model loading and GPU behavior are tied to the development machine | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F27 | The evaluation harness cannot block a bad release | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F28 | The installer does not contain a complete offline runtime | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F29 | Threat protection helpers report safety without security evidence | Verified mitigation | Unavailable providers fail closed; provider integrations remain capability work. |
| F30 | Laya metadata decisions are not an antivirus system | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F31 | Sandbox and installation helpers can claim actions that never happened | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F32 | Broker controls are disconnected and API endpoints can redirect credentials | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F33 | Persistence and orchestration add deployment complexity without serving the desktop | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F34 | Parsing and semantic search are neither safe nor complete yet | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F35 | History and background tasks do not support a dependable daily assistant | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F36 | Approval views do not expose every planned item or bind all actions consistently | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F37 | Journal size limits are checked only on opening | Verified mitigation | Append checks the resulting journal size. |
| F38 | Replay prevention and wire schemas are not yet a complete process contract | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F39 | Documentation and release verification can overstate readiness | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F40 | Conversation state and observations can diverge from current task truth | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F41 | “My system” is silently narrowed to personal folders | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F42 | General file operations are absent from the live capability catalogue | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
| F43 | Agent ping readiness does not establish actual model readiness | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F44 | Interface success signals are not tied to verified outcomes | Open | Requires focused implementation/review and acceptance evidence before closure. |
| F45 | Current Laya decisions differ from the original escalate-only design | Verified mitigation | ML recommendations cannot approve or authorize deletion. |
| F46 | Side-by-side work lacks an integrated durable task coordinator | Partially corrected | Requires focused implementation/review and acceptance evidence before closure. |
