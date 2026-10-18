# Capability register

Generated from [CAPABILITY-REGISTER.json](CAPABILITY-REGISTER.json). That JSON is the canonical register.

Statuses describe stated subsets. No row establishes whole-product acceptance. Component ownership is by module; no staffing is assumed.

| ID | Domain / capability | Current status | Implementation owner | Gate | Dependencies |
| --- | --- | --- | --- | --- | --- |
| D01 | desktop: Launch fixed local apps | implemented_unverified | desktop fixed launch / paused broad app actions | Stage 4 | S01, S03 |
| D02 | desktop: Discover/list/close permitted applications | paused | desktop fixed launch / paused broad app actions | Stage 4 | S03 |
| D03 | desktop: Observe, click, type and navigate controls | missing | desktop fixed launch / paused broad app actions | Stage 4 | S01, S03, P01 |
| D04 | desktop: Manage windows, multitask and notifications | missing | desktop fixed launch / paused broad app actions | Stage 4 | D03 |
| F01 | files: Search arbitrary permitted local folders/drives | verified_subset | desktop assistant / core search | Stage 5 | S02 |
| F02 | files: Read file metadata | implemented_unverified | desktop assistant / custodian-core file tasks | Stage 5 | S02 |
| F03 | files: Move/copy/rename/create folder with exact approval | verified_subset | desktop assistant / custodian-core file tasks | Stage 5 | S02, S04 |
| F04 | files: Automatic single largest-file copy | verified_subset | desktop routine / native backup | Stage 5 | S04 |
| F05 | files: Bounded document/image collection compositions | verified_subset | desktop workflow / sandboxed Qwen / native core | Stage 5 | S04, P02 |
| F06 | files: General batch organization and tree operations | partial (core manifest only; route open) | desktop assistant / custodian-core file tasks | Stage 5 | P01, S05 |
| F07 | files: Compress/extract archives | partial (bounded core ZIP only; route open) | custodian-core batch archive primitive | Stage 5 | S01, F06 |
| F08 | files: Versioned backup and restore | partial (core version store; route open) | custodian-core versioned backup | Stage 5 | F06, S05 |
| F09 | files: Governed delete and disk cleanup | paused (provider unavailable) | native privacy and tool dispatcher | Stage 5 | S01, S05 |
| F10 | files: Unicode/long paths, unavailable drives and locked files | partial | desktop assistant / custodian-core file tasks | Stage 5 | S05 |
| C01 | content: Grounded local content extraction and summarization | missing | unconnected parser/reader and missing artifact executor | Stage 6 | S02, S07 |
| C02 | content: Create/edit text, Markdown and structured CSV | missing | unconnected parser/reader and missing artifact executor | Stage 6 | S01, P01 |
| C03 | content: Create/edit DOCX and PDF | missing | unconnected parser/reader and missing artifact executor | Stage 6 | C01, D03 |
| C04 | content: Create/edit spreadsheets and formulas | missing | unconnected parser/reader and missing artifact executor | Stage 6 | C02, D03 |
| C05 | content: Create/edit presentations | missing | unconnected parser/reader and missing artifact executor | Stage 6 | C01 |
| C06 | content: Image/media transformation, OCR and conversion | missing | unconnected parser/reader and missing artifact executor | Stage 6 | S07 |
| P01 | planning: General heterogeneous task engine | partial | core coordinator / desktop | Stage 2 | S01, S05, S06 |
| P02 | planning: Qwen typed capability proposals | verified_subset | desktop workflow / sandboxed Qwen / native core | Stage 3 | S01 |
| P03 | planning: Natural wording, references, exclusions and ambiguity | partial | sandboxed agent / native IPC | Stage 3 | P01 |
| P04 | planning: Observe/replan/retry unfamiliar multi-app tasks | missing | core coordinator / desktop | Stage 6 | P01, P03, D03, C01 |
| P05 | planning: Laya semantic reasoning/checking integration | prototype | experimental ML host only | Stage 3 | P03, S01 |
| T01 | settings: Six per-user settings and revert | implemented_unverified | desktop settings / Windows catalog | Stage 7 | S01, S04 |
| T02 | settings: Audio, display and power controls | missing | desktop settings / Windows catalog | Stage 7 | S01, T01 |
| T03 | settings: Bluetooth, local adapters, defaults and peripherals | missing | desktop settings / Windows catalog | Stage 7 | S01, T01 |
| T04 | settings: Fault diagnosis, processes and startup apps | partial | desktop fixed launch / paused broad app actions | Stage 7 | S01, D02 |
| V01 | development: Project-scoped code editing and local Git | missing | unsupported development/elevation execution | Stage 8 | S01, S07 |
| V02 | development: Contained commands, builds, tests and debugging | missing | unsupported development/elevation execution | Stage 8 | V01, S07 |
| V03 | development: Offline install/configure/update/uninstall | missing | unsupported development/elevation execution | Stage 8 | S01, S05, S07 |
| R01 | routines: Local reminders and to-do lists | implemented_unverified | desktop local task store | Stage 9 | S01 |
| R02 | routines: Reusable scheduled workflows and preferences | partial | desktop routine / native backup | Stage 9 | P01, S04 |
| R03 | routines: Local email drafts, ICS and contact files | missing | unconnected parser/reader and missing artifact executor | Stage 9 | C02, S02 |
| R04 | routines: Scoped proactive automation | missing | core coordinator / desktop | Stage 9 | R02 |
| I01 | interaction: Local text chat and natural task interaction | partial | sandboxed agent / native IPC | Stage 3 | P03 |
| I02 | interaction: Accessible beginner task interface | partial | desktop React controls and native task commands | Stage 10 | P01 |
| I03 | interaction: Multilingual task execution | missing | sandboxed agent / native IPC | Stage 10 | P03 |
| I04 | interaction: Optional offline voice | deferred | ML host voice placeholder | Stage 10 | I01, S03 |
| I05 | interaction: Progress, task-specific Stop and safe pause | partial | core coordinator / desktop | Stage 2 | S05 |
| I06 | interaction: Predictable startup and resource usage | partial | sandboxed agent / native IPC | Stage 10 | P01 |
| S01 | security: Independent policy for every API/UI/script effect | partial | native privacy and tool dispatcher | Stage 1 | None |
| S02 | security: Private paths and default credential exclusions | verified_subset | native privacy and tool dispatcher | Stage 1 | None |
| S03 | security: Private apps/accounts/screens before capture | missing | native privacy and tool dispatcher | Stage 1 | S02 |
| S04 | security: Scoped grants and consequence-specific approval | partial | native privacy and tool dispatcher | Stage 1 | None |
| S05 | security: Real crash recovery and truthful completion | partial | desktop workflow / sandboxed Qwen / native core | Stage 1 | None |
| S06 | security: Cross-process authority and resource coordination | verified_subset | core coordinator / desktop | Stage 1 | S04 |
| S07 | security: Injection/path/plugin/command/exfiltration resistance | partial | native privacy and tool dispatcher | Stage 11 | S01, S03 |
| S08 | security: Contained local model with no authority escalation | verified_subset | sandboxed agent / native IPC | Stage 1 | None |
| S09 | security: Risk tiers, critical-action refusal and exact elevation | partial | unsupported development/elevation execution | Stage 1 | S01 |
| S10 | security: Audit/receipt integrity and signed release/update chain | partial | Windows Tauri bundle/runtime staging | Stage 12 | S01, S05 |
| Q01 | deployment: Pinned offline Windows runtime/package | verified_subset | Windows Tauri bundle/runtime staging | Stage 12 | None |
| Q02 | deployment: Clean offline install, upgrade, restart and uninstall | missing | Windows Tauri bundle/runtime staging | Stage 12 | Q01, S05 |
| Q03 | deployment: Independent workflow and beginner evaluation | missing | desktop React controls and native task commands | Stage 12 | P04, Q02 |
| N01 | online: Web research, forms, transfers and connected services | deferred | offline gate / unavailable broker | Deferred | S01, S03 |
| N02 | online: Email, messaging, calendar, meetings and contacts accounts | deferred | offline gate / unavailable broker | Deferred | N01 |
| X01 | platform: macOS/Linux native operator releases | deferred | Windows Tauri bundle/runtime staging | Deferred | Q03 |

## Traced routes

### files: desktop assistant / custodian-core file tasks

send_chat_message -> respond -> execute/execute_inner -> file_ops prepare_call -> exact approval -> FileTasks execute_plan -> sealed receipt -> undo/verification

- [apps/desktop/src-tauri/src/assistant/mod.rs](../apps/desktop/src-tauri/src/assistant/mod.rs)
- [apps/desktop/src-tauri/src/assistant/tools.rs](../apps/desktop/src-tauri/src/assistant/tools.rs)
- [apps/desktop/src-tauri/src/assistant/file_ops.rs](../apps/desktop/src-tauri/src/assistant/file_ops.rs)
- [crates/custodian-core/src/file_tasks.rs](../crates/custodian-core/src/file_tasks.rs)

### discovery: desktop assistant / core search

respond -> native intent or validated model ToolCall -> execute -> current privacy scope -> run_find/run_largest -> measured native result

- [apps/desktop/src-tauri/src/assistant/mod.rs](../apps/desktop/src-tauri/src/assistant/mod.rs)
- [crates/custodian-core/src/find.rs](../crates/custodian-core/src/find.rs)
- [crates/custodian-core/src/largest.rs](../crates/custodian-core/src/largest.rs)
- [crates/custodian-core/src/walk.rs](../crates/custodian-core/src/walk.rs)
- [crates/custodian-core/src/privacy.rs](../crates/custodian-core/src/privacy.rs)

### workflow: desktop workflow / sandboxed Qwen / native core

respond -> workflow::handle -> measured masked location bindings -> authenticated intent_plan -> Qwen semantic capability/role choice -> native constraint/dependency compilation -> current grant -> coordinated verified copies -> receipts/inspect/undo; legacy workflow_plan retained

- [apps/desktop/src-tauri/src/assistant/workflow.rs](../apps/desktop/src-tauri/src/assistant/workflow.rs)
- [services/agent/src/workflow-plan.ts](../services/agent/src/workflow-plan.ts)
- [services/agent/src/llm.ts](../services/agent/src/llm.ts)
- [crates/custodian-core/src/workflow.rs](../crates/custodian-core/src/workflow.rs)
- [crates/custodian-core/src/routine.rs](../crates/custodian-core/src/routine.rs)
- [apps/desktop/src-tauri/src/assistant/language.rs](../apps/desktop/src-tauri/src/assistant/language.rs)
- [services/agent/src/intent-plan.ts](../services/agent/src/intent-plan.ts)
- [crates/custodian-core/src/intent_plan.rs](../crates/custodian-core/src/intent_plan.rs)

### backup: desktop routine / native backup

respond -> routine::handle -> default-off standing grant -> prepare_backup -> execute_backup -> FileTasks receipt -> verified undo

- [apps/desktop/src-tauri/src/assistant/routine.rs](../apps/desktop/src-tauri/src/assistant/routine.rs)
- [crates/custodian-core/src/routine.rs](../crates/custodian-core/src/routine.rs)
- [crates/custodian-winsec/src/guarded_copy.rs](../crates/custodian-winsec/src/guarded_copy.rs)

### privacy: native privacy and tool dispatcher

PrivacyPanel native commands -> AccessPolicy -> execute unavailable gate/with_policy -> boundary checks; paths and credential exclusions exist, app/screen zones do not

- [apps/desktop/src-tauri/src/assistant/privacy.rs](../apps/desktop/src-tauri/src/assistant/privacy.rs)
- [crates/custodian-core/src/privacy.rs](../crates/custodian-core/src/privacy.rs)
- [apps/desktop/src-tauri/src/assistant/mod.rs](../apps/desktop/src-tauri/src/assistant/mod.rs)

### apps: desktop fixed launch / paused broad app actions

OpenApp fixed enum -> execute -> approval when required -> open_app_tool; OpenProgram/ListOpenApps/CloseApp -> privacy::unavailable -> paused; no general accessibility executor

- [apps/desktop/src-tauri/src/assistant/mod.rs](../apps/desktop/src-tauri/src/assistant/mod.rs)
- [apps/desktop/src-tauri/src/assistant/privacy.rs](../apps/desktop/src-tauri/src/assistant/privacy.rs)
- [apps/desktop/src-tauri/src/assistant/launcher.rs](../apps/desktop/src-tauri/src/assistant/launcher.rs)
- [crates/custodian-winsec/src/apps.rs](../crates/custodian-winsec/src/apps.rs)

### settings: desktop settings / Windows catalog

Settings intent/catalog resolution -> ChangeSetting -> approval -> change_setting_tool/apply_setting_change -> cataloged HKCU write + history; revert compares saved/current values; live desktop acceptance open

- [apps/desktop/src-tauri/src/assistant/mod.rs](../apps/desktop/src-tauri/src/assistant/mod.rs)
- [apps/desktop/src-tauri/src/assistant/settings.rs](../apps/desktop/src-tauri/src/assistant/settings.rs)
- [crates/custodian-winsec/src/settings.rs](../crates/custodian-winsec/src/settings.rs)

### tasks: desktop local task store

typed AddTask/ListTasks/CompleteTask/RemoveTask -> persisted local task list; reminders require app running or reopening; not an external scheduler

- [apps/desktop/src-tauri/src/assistant/tasks.rs](../apps/desktop/src-tauri/src/assistant/tasks.rs)
- [apps/desktop/src-tauri/src/assistant/when.rs](../apps/desktop/src-tauri/src/assistant/when.rs)
- [apps/desktop/src-tauri/src/assistant/mod.rs](../apps/desktop/src-tauri/src/assistant/mod.rs)

### coordinator: core coordinator / desktop

desktop collection -> native bounded task program -> dependency facts -> per-step native authorization -> write-ahead checkpoint -> existing receipt-backed file executor -> explicit verification; pause at program boundaries; mock app composition tested only; alternate orchestrator remains unavailable

- [crates/custodian-core/src/coordinator.rs](../crates/custodian-core/src/coordinator.rs)
- [crates/custodian-core/src/task_program.rs](../crates/custodian-core/src/task_program.rs)
- [crates/custodian-core/src/workflow.rs](../crates/custodian-core/src/workflow.rs)
- [apps/desktop/src-tauri/src/assistant/workflow.rs](../apps/desktop/src-tauri/src/assistant/workflow.rs)
- [apps/desktop/src-tauri/src/assistant/mod.rs](../apps/desktop/src-tauri/src/assistant/mod.rs)
- [apps/desktop/src/components/TaskBoard.tsx](../apps/desktop/src/components/TaskBoard.tsx)
- [services/orchestrator/src/activities.ts](../services/orchestrator/src/activities.ts)

### content: unconnected parser/reader and missing artifact executor

readerExtract explicitly returns available:false; no source-grounded content/artifact tool chain in current desktop catalog; parser existence does not establish connected ingestion

- [services/agent/src/reader.ts](../services/agent/src/reader.ts)
- [crates/custodian-parser/src/main.rs](../crates/custodian-parser/src/main.rs)
- [services/agent/src/tools.ts](../services/agent/src/tools.ts)

### model: sandboxed agent / native IPC

respond -> sandboxed HMAC chat or isolated bounded intent_plan history -> semantic selection -> independent native validation; model supplies no paths or permission

- [apps/desktop/src-tauri/src/agent.rs](../apps/desktop/src-tauri/src/agent.rs)
- [services/agent/src/server.ts](../services/agent/src/server.ts)
- [services/agent/src/llm.ts](../services/agent/src/llm.ts)
- [services/agent/src/tool-plan.ts](../services/agent/src/tool-plan.ts)
- [apps/desktop/src-tauri/src/assistant/language.rs](../apps/desktop/src-tauri/src/assistant/language.rs)
- [services/agent/src/intent-plan.ts](../services/agent/src/intent-plan.ts)
- [crates/custodian-core/src/intent_plan.rs](../crates/custodian-core/src/intent_plan.rs)

### laya: experimental ML host only

standalone evaluator -> typed semantic results; failed activation criteria; no desktop authorization route

- [services/mlhost/mlhost/action_screen.py](../services/mlhost/mlhost/action_screen.py)
- [services/mlhost/mlhost/typed_decisions.py](../services/mlhost/mlhost/typed_decisions.py)
- [services/mlhost/mlhost/laya_compare.py](../services/mlhost/mlhost/laya_compare.py)

### scripts: unsupported development/elevation execution

no desktop command/project/install tools; legacy sandbox argument expansion does not establish restricted script execution; unsupported elevation verbs fail

- [crates/custodian-elevate/src/main.rs](../crates/custodian-elevate/src/main.rs)
- [crates/custodian-sandbox/src/lib.rs](../crates/custodian-sandbox/src/lib.rs)
- [services/agent/src/tools.ts](../services/agent/src/tools.ts)

### online: offline gate / unavailable broker

WebSearch/OpenWebsite -> offline refusal before handoff; BrowserBroker start/performAction throw unavailable; no production online workflow

- [apps/desktop/src-tauri/src/assistant/privacy.rs](../apps/desktop/src-tauri/src/assistant/privacy.rs)
- [services/broker/src/index.ts](../services/broker/src/index.ts)

### voice: ML host voice placeholder

stt uses a byte marker or empty text; tts returns empty bytes; not real speech inference

- [services/mlhost/mlhost/voice.py](../services/mlhost/mlhost/voice.py)

### ui: desktop React controls and native task commands

chat/mascot controls -> Tauri commands -> native progress/cancel; UIA harness is an external test driver, not an assistant app-control capability

- [apps/desktop/src/App.tsx](../apps/desktop/src/App.tsx)
- [apps/desktop/src/cubby/CubbyStage.tsx](../apps/desktop/src/cubby/CubbyStage.tsx)
- [apps/desktop/src/components/RoutineControls.tsx](../apps/desktop/src/components/RoutineControls.tsx)
- [scripts/verify-desktop.ps1](../scripts/verify-desktop.ps1)

### package: Windows Tauri bundle/runtime staging

stage pinned runtime -> Tauri NSIS bundle; existing unsigned installer requires WebView2; clean install/upgrade/uninstall acceptance absent

- [apps/desktop/src-tauri/tauri.conf.json](../apps/desktop/src-tauri/tauri.conf.json)
- [apps/desktop/src-tauri/windows/offline-prerequisite.nsh](../apps/desktop/src-tauri/windows/offline-prerequisite.nsh)
- [scripts/stage-runtime.mjs](../scripts/stage-runtime.mjs)
- [models/manifest.json](../models/manifest.json)

## Frozen acceptance tasks

Each of the 58 requirements has a task and expected result in the canonical JSON.
The [fixture specification](STAGE-0-ACCEPTANCE-FIXTURES.json) contains 15 representative authored cases with fixed source text and expected effects.
Their [acceptance lock](EVIDENCE/stage-0/acceptance-lock.json) detects expectation changes; none of these specifications is automatically labelled an executed result.
