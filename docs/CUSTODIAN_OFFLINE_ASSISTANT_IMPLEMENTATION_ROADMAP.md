# Custodian: from desktop companion to dependable offline assistant

**Detailed implementation roadmap and extension to the project audit**  
**Date:** 14 October 2026  
**Audience:** Project owner, developers, reviewers, and anyone joining the project  
**Status:** Proposed implementation programme grounded in the current source and original design. This document does not mean the described fixes have been implemented.

[Open the extended project audit and source findings](CUSTODIAN_PROJECT_AUDIT_2026_10_14.html).

## 1 The outcome we are trying to achieve

Custodian should let a person delegate useful work to their Windows PC in ordinary language, continue their own work while that task runs, and see a trustworthy result. It should find files, explain their actual properties, perform supported file operations and settings changes, and provide an understandable way back when a reversible operation goes wrong. Its core capabilities should work without an Internet connection. Its authority should be limited by the user's choices and by deterministic safety rules, rather than by whatever the language model happens to say.

The user's examples are the right acceptance criteria. “Find the largest file on my system” must inspect the intended scope and report an actual file and measured size. “Move it to this folder” must identify the same file, obtain any necessary authority, perform the operation, verify the result, and report where it went. “Undo that” must refer to the right completed task and handle changes made since that task. The assistant must be able to do this while the person keeps using the PC.

The current build does not yet deliver this consistently. The primary problem is the incomplete connection between conversation, observations, permission, execution, recovery, and the interface. Changing the model or adding stronger prompts alone will not repair that connection.

### 1.1 The desired interaction, in plain language

1. You ask for a result, such as the largest file across your local drives.
2. Custodian shows what it will search and starts a read-only task under your configured access rules.
3. You can continue typing, working in another app, or giving an independent command.
4. Custodian returns measured results with scope, time, and any unreadable or excluded locations.
5. You ask it to move one result. Custodian refers to the selected file's identity, not a guessed filename.
6. It checks that the file has not changed, checks the destination, and prepares recovery before making the change.
7. It uses an existing valid task grant or asks for the specific approval still needed.
8. It performs the change and checks the outcome. The interface says “completed” only after that check.
9. History records the result. Undo is available where its preconditions still hold.

### 1.2 What “all tasks” should mean

No safe implementation should interpret “all tasks” as unrestricted shell access, universal administrator rights, or a promise that every application and Windows setting is automatable. The practical goal is a growing catalogue of supported, tested capabilities. Each capability declares what it can read, change, verify, and undo. Unsupported requests receive an honest explanation and a useful handoff; they must not receive a fictional success message.

The original design explicitly excludes automating some high-risk categories, including security settings, drivers, root certificates, credential actions, payments, and other sensitive external actions. It also describes Custodian as not being a general system tweaker. Harmless appearance or other explicitly supported personal settings belong in the catalogue; disabling security protections does not become allowed merely because the model understands the request. Any broader product scope needs an explicit revision of the original decision register, with a threat model and tests.

## 2 Sources, review boundaries, and how to read this document

This roadmap extends `D:\Custodian\docs\CUSTODIAN_PROJECT_AUDIT_2026_10_14.md`, which records 39 initial findings, their source evidence, corrective actions, and 55 acceptance cases. The audit extension adds F40–F46 for the symptoms and design gaps examined here. Use the audit for the defect inventory and this roadmap for the sequence and detailed target behavior.

### 2.1 Original documents supplied by the project owner

| Document | Original location | Purpose in this review |
|---|---|---|
| Architecture | `C:\Users\paagu\OneDrive\Desktop\ideas\CUSTODIAN_ARCHITECTURE.md` | Process boundaries, authority, contracts, model roles, failure handling and resource targets |
| Build plan | `C:\Users\paagu\OneDrive\Desktop\ideas\CUSTODIAN_BUILD_PLAN.md` | Original 170 implementation steps and their verification gates |
| Master document | `C:\Users\paagu\OneDrive\Desktop\ideas\CUSTODIAN_MASTER_DOCUMENT.md` | Product intent, user journeys, safety invariants, evidence and reversibility |
| Original document index | `C:\Users\paagu\OneDrive\Desktop\ideas\README.md` | Canonical document relationship and synchronization rule |

The three supplied design documents were compared with their counterparts in `D:\Custodian\docs\design`. Their line-by-line text matches after newline normalization. Byte hashes differ, but the comparison did not find substantive text drift. The important drift is between the design and the running implementation. This roadmap does not edit the originals or silently replace their canonical decisions.

Read Master sections 8–10 for the product and reversibility boundaries; sections 19 and 21–25 for onboarding, evidence, finding, Later, stop and voice; and section 73 for offline tasks. Architecture A11 describes model contexts and tools; A13 durable workflows; A14 sandboxed file tasks; A22 error handling; A24 performance. The canonical D-01–D-27 register and build-step numbers are the reference when adopting design changes.

### 2.2 Evidence labels

| Label | Meaning |
|---|---|
| Confirmed in source | The inspected code demonstrates the implementation or missing connection. This does not imply every consequence was reproduced in a live installed session. |
| User-reported symptom | A problem experienced by the user; its exact instance still needs a trace to attribute to a particular branch. |
| Capability gap | The requested functionality is missing, incomplete, or not connected to the desktop route. |
| Proposed design | The behavior and implementation recommended here; it must still be built and tested. |
| Verified previously | A check actually executed during the initial audit. Its limitations remain in that audit's verification addendum. |

No destructive operation on personal files, real settings changes, antivirus test attack, or live whole-PC scan was performed for this extension. This is not a promise that every defect has been discovered. The earlier automated checks are useful evidence about the codebase, not proof of clean-machine readiness or the user's exact runtime problem.

## 3 What the original plan gets right and what is currently missing

The original plan describes a much more disciplined product than an unrestricted chat bot controlling a computer. Keep its central ideas: a model proposes actions; a deterministic kernel makes policy decisions; a dedicated Engine performs writes; exact approvals are signed; recovery precedes mutation; untrusted documents are treated as data; and the Internet is available only through a controlled, explicitly enabled path.

| Original commitment | Current gap or risk | Required direction |
|---|---|---|
| Offline core, D-04/D-08 | Offline policy is not enforced at every live network handoff; installation is incomplete | Make offline a centrally enforced state and ship all core runtime assets |
| Engine is the only writer, D-07 | Desktop paths perform some changes directly; engine and journal exist partly as separate building blocks | Migrate every supported mutation to one guarded execution route |
| Way back before action, D-06 | Live cleanup and settings routes have write/recovery ordering gaps | Persist preparation and inverse information before any real change |
| Exact, isolated approval, D-14 | Approval binding, UI coverage and replay behavior have gaps | Bind object identities, versions, destination, risk and budgets to one plan |
| Scoped Mastra tools, D-12 | Live model/tool flow differs from parts of the planned framework | Preserve the typed-tool security boundary; formally decide framework differences |
| Memory and evidence, Master 21–23/A11 | Conversation, direct tool results and approval outcomes are not one coherent task history | Separate durable task truth, fresh observations, preferences and model context |
| Real background workflows, D-11/A13 | Reminders, UI state and workflow scaffolding are not a complete task coordinator | Implement durable task lifecycle, retry rules, locks and resource scheduling |
| Laya only adds caution, D-19 | Current metadata chat classifier returns approve/delete/quarantine | Restore escalate-only semantics; no AI verdict may authorize deletion |
| Full offline installer, D-21 | Developer-machine success does not establish a complete installed runtime | Gate release on offline clean-VM installation and actual tasks |
| Evidence-backed claims, D-27 | Some placeholder and UI paths can imply success without verified work | Make every operational claim derive from a typed result and evidence |

The original plan is ambitious: multiple isolated processes, database, workflow runtime, models, voice, sandboxed transformations, online browsing, installations and helper pairing. Trying to finish all of these at once makes it easy to accumulate stubs without a reliable user journey. The roadmap below delivers vertical slices: each slice starts at a real user command and ends with a verified outcome, safety checks and recovery.

## 4 Diagnosis of the four problems you reported

### 4.1 “It answers from pre-saved memory”

**User report:** Answers sometimes appear to come from old information instead of inspecting the PC.

**Confirmed in source:** `services/agent/src/llm.ts` reuses a `LlamaChatSession` and calls `chat.prompt` for successive model-mediated turns. It has a `reset()` method, but the live protocol in `services/agent/src/server.ts` has chat, tool-result, cancel and ping messages, without an explicit conversation-reset or conversation identity contract. A reset used by an evaluation harness is not a live session-management system.

There are also two forms of divergence. Some recognized commands are executed directly in `apps/desktop/src-tauri/src/assistant/mod.rs` and do not enter the model's history. Approval-card outcomes are added to the visible conversation by `apps/desktop/src/App.tsx`, but are not consistently fed back into that same model context as authoritative completed or rejected events. Thus the UI can know something that the model does not. Conversely, the model can retain an earlier tool result and reuse it after the real file or setting has changed.

The current tool-result protocol transports a string rather than a structured observation with object identity, version, scope and observation time. The tool-call budget also ends with an instruction to answer using what is already known. That does not force a new observation for a question about the PC right now. Existing action-claim correction is conditional on no tool having been called, so an irrelevant or unsuccessful tool call does not establish the truth of the answer.

**What is not established:** The review has not proved that a disk-backed long-term memory database is causing the reported incident. Possible contributors include in-process history, incomplete model context, canned fallback text, stale observations, failed tools, or the model's own unsupported inference. Trace the affected request before assigning a single cause. Deleting every memory file or changing the model would not repair the underlying contracts.

**Resolution:** Implement the observation and conversation design in section 5. Operational facts require current evidence. History may help interpret a request, but may not stand in for a new file check or a verified execution receipt.

### 4.2 “It cannot find the largest file in my system properly”

**Confirmed in source:** `Scope::UserFolders` in `assistant/intent.rs` is explicitly described as what “my system” means. The supported known folders are Downloads, Documents, Desktop, Pictures, Videos and Music. `run_largest` requests a top ten from those resolved folders, even when the person asks for one largest file.

The walker in `crates/custodian-core/src/walk.rs` excludes many directories, including AppData, Windows, Program Files, dependency/build directories and dot-prefixed folders. It skips reparse/offline entries, quietly skips unreadable directories, and has a 400,000-file budget. The response path does not fully present those limits and coverage conditions. A correctly implemented top-K algorithm cannot return a file that the selected scope and walker never considered.

This is primarily a scope, coverage and reporting defect, not evidence that sorting file sizes is fundamentally broken. A file on another drive, outside the six known folders, inside a skipped dependency tree, or beyond the budget may be larger than every reported result. The user also needs to know whether “size” means logical file length or space actually occupied on disk.

**Resolution:** Introduce an explicit query scope and size metric, enumerate authorized local volumes for whole-system queries, separate read-only metadata access from mutation policy, report coverage honestly, and use a fresh scan or defensible index freshness model. Section 6 specifies the algorithm and user experience.

### 4.3 “It cannot make changes in my system and files”

**Confirmed capability gap:** The current `ToolCall` catalogue in `assistant/tools.rs` has discovery, selected application actions, tasks, cleanup, undo and a limited settings interface. It does not expose a general implemented create/copy/move/rename/edit-file capability set. The current settings catalogue supports six settings; opening the Windows Settings application for an unsupported request is a handoff, not the requested change.

Some existing mutations do run, but the audit found recovery, verification, authority and concurrency gaps. Expanding that code into arbitrary file operations before fixing the execution route would expand the possible damage. This is not solved by granting the entire application administrator access or allowing model-generated PowerShell.

**Resolution:** Build one safe mutation transaction and implement a small catalogue of real operations on top of it. Add capability availability reporting so the assistant distinguishes unsupported, blocked, waiting for approval, failed and completed. Sections 7–9 define the changes.

### 4.4 “It should work alongside me, not only speak or type”

**Confirmed gap:** The desktop's `isThinking` state and single pending-plan slot are conversation-oriented. The backend also has global cancellation and serialized agent use. These are not a durable, independently controllable task system. Some long operations cannot be interrupted promptly, and task outcomes are not the sole source of UI success state.

The frontend reacts happily when a chat invocation resolves and when an action was allowed. Neither event proves that a requested operation succeeded. Permission, transport success and task completion must be separate states.

**Resolution:** Add a task coordinator and outcome-driven interface. The model may remain serialized to save memory while independent approved tool tasks run under resource and file locks. A long scan must not prevent a short status request, a stop command or continued use of the PC. See section 10.

## 5 Repair memory, conversation and factual accuracy

### 5.1 Use five distinct kinds of information

| Information | Example | Lifetime and authority |
|---|---|---|
| General model knowledge | How folders work | Useful for explanation; never proof of this PC's state |
| Conversation context | “it” refers to the file selected in the previous result | Scoped to a conversation/task; resolves references, grants no permission |
| User preferences | Language, voice, protected folders | Persist only supported keys; editable and removable by the user |
| Observations | File A measured as 8 GiB at a particular time | Bound to source, identity, scope and freshness; invalidated by relevant changes |
| Task receipts/history | Task T moved file A to destination B and verified it | Durable record of what happened; does not imply the current file remains unchanged |

The original architecture's `remember_preference` tool is an allowlisted preference mechanism. Do not turn it into “remember any sentence the model says.” Protected-path preferences need policy validation; a model must not remove protection by writing memory. File facts, permissions and security verdicts are not ordinary preferences.

### 5.2 Make operational answers depend on observations

Add a structured observation object from Core. It should include an observation ID, producing capability, task ID, object references, start/end time, scope, policy version, completeness state, per-category omissions, source version and result data. Use monotonic elapsed time for in-process deadlines and wall-clock time for presentation; clock changes must not silently extend grants or make purge eligible.

For “what is the size of this file?”, obtain metadata for that exact current object. For “is dark mode on?”, read the supported setting. For “did you move it?”, read the task's verified receipt, and separately check current location if the question concerns its state now. For “what was the largest file yesterday?”, historical evidence is legitimate only if that historical observation exists and is labelled historical.

Do not use a universal five-minute cache as a correctness guarantee. Different questions need different evidence. A file may change immediately after a scan. For a mutation, revalidate at execution against the approved object version. For an approximate dashboard, a clearly labelled recent cached value may be acceptable. For an exact whole-scope maximum, see section 6.5.

### 5.3 Unify all conversation routes

Create a conversation event stream owned by the coordinator, independent of the model process. Record the user's request, normalized intent, selected references, observation summaries, proposed plan, approval decision, execution state and verified outcome. Both deterministic commands and model-mediated commands use this stream.

The UI renders that stream. The model receives a bounded, policy-filtered projection of it. When the user approves or declines, or when a tool finishes after the original message, feed that authoritative event into the appropriate task context before the next model turn. Do not reconstruct task truth by parsing assistant prose.

Add explicit conversation and task identifiers to IPC. “New conversation” clears conversational context without erasing audit/recovery history. Model unload/restart rebuilds only the necessary current context from structured events. Replaying events into the model must never replay their side effects.

Resolve references such as “that one,” “the second file” and “move it” against visible result IDs and conversation selection. If two plausible objects remain, ask a targeted clarification. Do not scan for a matching basename and silently choose another file.

### 5.4 Validate answers before declaring facts

Use deterministic templates for sizes, dates, paths, counts, task states and security-health fields. The model may explain a verified result in plain language, but factual clauses must map to evidence. A successful tool invocation is insufficient: check the tool's status and whether its evidence supports the claim.

For operational responses, buffer or constrain the final claim until evidence is available. Status messages such as “I am scanning” may stream while the task runs. “I moved the file” may appear only after the completed receipt. If verification fails, use the truthful status even if the model produced fluent success text.

When a tool budget is exhausted, return a bounded state such as “I have not completed this task; these two checks remain.” The coordinator can schedule a further bounded step if the task grant permits it. Never instruct the model to infer missing live facts from old context.

### 5.5 Diagnose the user's stale-answer incidents safely

Add a local diagnostic view with request ID, route chosen, capability invoked, observation timestamp, scope, tool result status, model availability and final evidence IDs. Store sensitive paths as protected object references or redacted labels in general logs. Detailed local diagnostics should be opt-in with clear retention, not an upload of private conversations.

A useful reproduction is: create a fixture file, ask its size, modify it externally, then ask again in the same chat. Repeat after a direct-command result, an approval outcome, a model idle unload, a new conversation and a restart. Record which layer returned the wrong state. This distinguishes stale observation reuse from missing conversational context.

## 6 Make “largest file in my system” accurate and useful

### 6.1 Define scope instead of guessing it

Use explicit scope types: selected file/folder, named known folders, chosen local volume, all authorized local fixed volumes, and separately authorized removable volumes. Network shares and cloud content are distinct scopes; “offline” must not trigger a remote fetch to satisfy a query.

“My system” should no longer silently become six personal folders. Under an established all-local-volumes metadata grant, search those volumes and show the scope. Without that grant, explain the available scope and offer a one-time expansion. Do not repeatedly ask for access already granted. Do not automatically request elevation just to make every folder readable.

Distinguish four sets: requested scope, policy-authorized scope, successfully scanned scope, and excluded/unavailable scope. An answer about a subset must say so. System areas may be eligible for read-only metadata under a chosen policy while remaining absolutely ineligible for automatic modification. Discovery eligibility and cleanup eligibility must not share one blanket exclusion list.

### 6.2 Specify the query contract

The normalized query should include the requested count, file-versus-folder target, size metric, scope references, link policy, cloud-placeholder policy, completeness requirement, cancellation token and resource budget. Singular “largest file” means one result; “ten largest” means ten. A folder-size request requires a separate aggregation capability and should not be silently treated as a file query.

Default to logical size when the person says “file size,” and label it. Show exact bytes plus a readable unit; explain GiB versus GB consistently. “What is taking up disk space?” needs allocated-size and deduplication rules, not simply logical length. Compressed/sparse files, hard links and cloud placeholders can make these metrics different. “How much space will deleting this free?” is another question and requires safe, operation-specific accounting.

### 6.3 Build the read-only scan pipeline

1. Resolve authorized volume/folder references through Core. Record stable volume identity and availability; do not assume every PC has only C: or that drive letters never change.
2. Normalize overlapping roots by identity and containment while keeping the broader root. Retain source-scope attribution for reporting.
3. Enumerate metadata without opening file contents. Handle hidden entries according to explicit query policy rather than dropping every dot-prefixed folder.
4. Handle links/reparse points without traversing arbitrary destinations. Record exclusions. A link must not expand a grant to another volume or create an infinite traversal.
5. Maintain a top-K heap using integer byte sizes. This retains a small candidate set rather than storing the entire tree for a one-off query.
6. Track successful entries, access-denied locations, policy exclusions, I/O failures, vanished entries, placeholders, budget limits and cancellation separately.
7. Recheck returned candidate identities and sizes before display, while preserving the scan interval and coverage caveat.
8. Return a typed observation with completion status and scope statistics. Use that result directly to render the answer and selectable file cards.

Use per-task cancellation and bounded concurrency. Progress reports should describe real work such as entries inspected and volumes completed. Do not invent a percentage before a denominator is known. A scan that reaches its safety budget should offer continuation or a narrower query; it must not present a partial maximum as exhaustive.

### 6.4 Report an answer that is actually defensible

Example of a complete-within-policy result: “The largest file found in the authorized local-drive scan was Archive.vhdx, 42.1 GiB. I searched C: and D: between 14:05 and 14:07. Protected locations were excluded.” The wording is tied to the observed interval and allowed scope, not an omniscient current claim.

Example of a partial result: “The largest file I could inspect was Archive.vhdx, 42.1 GiB. The scan stopped at its file limit, and 12 folders could not be read. There may be a larger file in those locations.” Give a way to continue, inspect exclusions or choose a drive.

Example after cancellation: “Stopped. Here are the largest files found so far.” The task is cancelled with partial observations; it is not completed. If results are empty because everything was denied, say that instead of “there are no files.”

### 6.5 Add an index without reintroducing stale answers

Start with correct live metadata scans. Add an index to accelerate repeated queries only after scope and coverage are explicit. Partition records by stable volume identity and file identity. Track index generation, scan coverage, last observation and change-monitor continuity. A watcher overflow, drive disconnection or unknown gap marks the affected scope stale and schedules reconciliation.

Rechecking only the previous top ten cannot prove that no newly created or enlarged file is bigger. Exact indexed maxima require a complete baseline and defensible change coverage, or a fresh reconciliation scan. If that cannot be established, label the answer as based on a recent index and offer a live scan. Even a live traversal spans time; a concurrently changing filesystem is not a single atomic snapshot. State that interval rather than claiming stronger consistency than the implementation provides.

### 6.6 Do not let a discovery result become write authority

A displayed result carries an opaque file reference. Asking to move it starts a separate mutation plan. Core resolves the reference and applies mutation policy; Engine reopens/revalidates the object. A user seeing a Windows system file in a size report does not mean Custodian may remove or alter it. Protected/private file names must also respect the person's configured read policy and safe display rules.

## 7 Add real file operations through one safe execution route

### 7.1 Minimum useful file catalogue

| Capability | Expected behavior | Required undo or limitation |
|---|---|---|
| Inspect file | Show measured size, type, time, location and supported attributes | Read-only; current observation required |
| Create folder | Create a named folder under an allowed parent | Remove on undo only if still the created object and safely empty |
| Create text file | Create new content with explicit encoding under an allowed parent | Remove only if unchanged; never silently replace an existing file |
| Copy file | Copy to an explicit destination and verify output | Remove unchanged created copy; source remains untouched |
| Rename file | Rename exact object without replacement by default | Restore name if identity/version and destination permit |
| Move file | Move exact object, handling same-volume and cross-volume cases explicitly | Recover location and supported metadata; preserve conflicts rather than overwrite |
| Edit text file | Preview a bounded content change, stage and validate new output | Retain original version; conflict if user edited after execution |
| Organize files | Preview a batch of individual supported operations | Per-item records and explicit partial-result/rollback behavior |
| Quarantine | Put eligible content in the safe place under policy | Authenticated recoverable manifest and no premature purge |
| Restore | Restore a chosen transaction or item | Never overwrite a newer user file without a new explicit decision |

Do not initially advertise arbitrary binary editing, every document format, recursive mass deletion or arbitrary application control. Add adapters after the basic transaction and recovery gates pass. Formatting, encoding, metadata and large-file limits must be explicit for each adapter.

### 7.2 The transaction every mutation must use

**Plan:** Normalize user intent into a typed operation. Resolve exact source and destination identities. Capture current state, proposed outputs, resource budget, risk class and inverse strategy. Reject unsupported cases before asking the user to approve something impossible.

**Authorize:** The deterministic policy checks protected roots, capability availability, grants and risk. A plan approval binds all material fields, including object versions, destination parent, operation parameters, expiry and budget. The model does not hold the signing key. Existing valid grants can cover bounded actions; material changes require revalidation and, where necessary, a revised approval.

**Prepare:** Acquire locks in a consistent order, reserve required space, create recovery material, validate the plan again and durably record the prepared transaction. A failure here means no mutation occurs. Approved plans are single-use, with durable replay handling appropriate to crash recovery.

**Execute:** Engine performs the allowlisted operation through typed APIs. No model-written shell, SQL, registry script or arbitrary command line. Use object-aware operations and destination validation so path substitution between check and use cannot redirect a write.

**Verify:** Check the intended state: identity, location, expected contents where relevant, and promised metadata. A return code alone is not sufficient. Hash copied content when required by the operation's integrity contract; do not hash every file merely to answer a size question.

**Commit:** Durably record the verified result and inverse before reporting completion. Use idempotent transaction state transitions so a restart can tell whether an operation was not started, partially done, verified or committed.

**Recover:** On restart, reconcile prepared/in-flight transactions before allowing conflicting work. If evidence is insufficient, hold for recovery; do not guess and repeat a destructive operation. History and recovery remain available when the model is unavailable.

### 7.3 Specific filesystem cases that must be designed

For same-volume moves, preserve the intended identity and metadata and handle name conflicts. For cross-volume moves, treat the operation as staged copy, verification, destination commit and controlled source removal with recovery, rather than assuming it is an atomic rename. The audit's current second-volume test limitation must be removed with a real second-volume test environment.

Declare support for ACLs, alternate data streams, timestamps, attributes, encrypted/compressed files and sparse files. If the implementation cannot preserve a required property, refuse or request a clearly explained reduced-fidelity copy mode; do not promise a perfect round trip. Unsupported filesystems and network destinations need explicit capability status.

For text edits, retain the original content, show the proposed change, validate encoding, and refuse stale preconditions. An editor may save a new version between planning and execution. Undo must detect subsequent changes; restoring an old backup over the user's newer work is not safe undo.

For batch operations, the preview must account for every item even if the visible list is paginated. Bind approval to the complete item set and counts. Define whether the batch stops on first failure or continues with independent items. “Rollback” is itself work that can fail; report rollback results separately and retain recoverable state.

### 7.4 Stop, undo and cleanup are different operations

Stop requests that no more forward work be scheduled and lets an in-flight atomic step reach a safe boundary. It does not promise instant reversal. Undo is a new authorized recovery operation with its own preconditions. Purge permanently discards recovery material and needs stronger policy and integrity checks. Keep these controls visually and technically distinct.

Do not purge because a cloud object merely has matching size and timestamp, because a timer expired while the workflow service was down, or because history appears empty after a missing-file error. The original design's “Temporal down means no purge” constraint remains unless formally replaced with an equally explicit fail-closed design.

## 8 Make system settings changes real, bounded and verifiable

Create a settings adapter interface: identify supported setting, inspect actual state, validate requested value, prepare exact inverse, apply, read back, and restore conditionally. Each adapter declares operating-system support, affected user/session scope, required privilege and whether a restart or application refresh is needed.

Persist the original raw state before applying a change. This includes distinctions such as an absent value versus an explicitly configured value and the value's type. “Default” is not always equivalent to the exact previous state. After applying, read the actual state and compare it with the intended value. A successful API call followed by an unchanged setting is an unverified/failed task, not success.

Undo should compare the current setting with the value Custodian last applied. If the person changed it afterward, do not overwrite that newer choice automatically. Explain the conflict and offer an explicit restore decision. Do not discard an undo record merely because a fixed-length history array became full; retention must honor the promised recovery policy.

Expand from the six current settings one adapter at a time. Suitable candidates depend on supported Windows APIs, tested OS versions and the original policy. Opening a settings page remains useful for unsupported settings, but the receipt must say “opened settings page; no setting changed.” Security controls, drivers, certificates and broad machine policy remain outside automatic control under the current canonical design.

Elevated operations, if later justified within scope, belong in the narrow Elevate helper with exact allowlisted verbs, signed requests, revalidation and explicit UAC. Do not run the model or everyday assistant permanently as administrator.

## 9 A capability registry that prevents imaginary tools

Maintain one versioned capability registry from which tool schemas, routing, UI availability descriptions and documentation are generated or checked. This addresses drift between Rust enums, TypeScript tools, model prompts and user-facing claims.

Each entry needs a stable ID and version; supported input/output schema; availability probe; risk class; read/write scope; offline/network classification; required grant; preconditions; preview builder; execution handler; verifier; inverse handler or irreversibility statement; cancellation boundaries; retry policy; resource budget; and evidence requirements. A declared capability without a working handler and verifier is unavailable, not experimental success.

Availability is device-specific. A installed package may lack a model, parser, Windows feature, privilege, disk space or supported setting. Distinguish not installed, unavailable on this device, temporarily busy, blocked by policy and operational. The language model receives only the relevant available tools and honest descriptions of unavailable requests.

The deterministic fast path and the model path must produce the same typed request and enter the same coordinator. They may differ in understanding natural language; they must not differ in authorization, transaction handling or completion evidence.

## 10 Let Custodian work alongside the person

### 10.1 Separate chat from task execution

Introduce a task coordinator with durable task IDs and independently addressable jobs. A task may outlive the chat turn that created it. Chat accepts a new request while a scan or approved conversion runs. The local model can process requests sequentially while the coordinator schedules safe independent tool work concurrently.

Do not solve concurrency by loading a separate large model per task. Maintain one bounded planner queue initially. Read-only metadata tasks use a small I/O pool. Mutations lock their exact resources and destination parents. Conflicting operations wait or explain the conflict. Independent tasks may proceed within CPU, memory and I/O budgets.

### 10.2 Define an honest task lifecycle

Use states such as queued, inspecting, needs-clarification, waiting-approval, preparing, running, verifying, completed, partial, cancelled, failed and recovery-required. Paused is a resumable state, not failure. Approval is an event that authorizes execution; it is not a completion state.

Persist allowed transitions and terminal evidence. The UI displays progress and receipts from this lifecycle. The assistant's animation, text, notifications and history should all agree. A successful IPC response carrying a task failure must produce failure UI, not a happy completion animation.

Each task has its own stop control. “Stop everything” cancels all eligible work and prevents queued forward actions. A stop request must be handled independently of waiting for the next model token. Show when an operation is finishing a safe atomic step and what changed before cancellation.

### 10.3 Design the everyday interface

Provide a compact task list with current action, progress when meaningful, pause/stop, needed decision, and eventual result. File results have selectable identity-backed cards and actions such as reveal, inspect and propose move. The full history shows what changed, when, and the available undo action. Multiple approvals belong to separate tasks; do not overwrite a single global pending-plan slot.

For background work, avoid stealing keyboard focus or moving the pointer. Prefer native task APIs. If a later application adapter needs accessibility/UI automation, require explicit targeting, detect unexpected application state, and pause when user interaction conflicts. Coordinate with the person rather than clicking while they are typing elsewhere. Do not introduce continuous screen or keystroke collection to implement “alongside me.”

Give useful answers without unnecessary confirmation for permitted read-only actions. For writes, use the configured Ask/Trusted/task-limited modes and valid existing grants. A person should not approve each file in an already approved bounded batch, nor should one approval authorize unrelated future tasks.

### 10.4 Implement Later as deferred work

The original Later feature is more than a reminder list. Store a deferred intent, trigger, scope, policy and expiry. At trigger time, inspect the current state, rebuild a plan, and obtain any required fresh approval. Do not execute yesterday's file references or expired grants blindly. Support pause, cancel, missed-trigger handling, restart recovery and clock-change safeguards.

Keep reminders that only notify distinct from deferred tasks that may act. The UI must show which one the user created. Durable orchestration must enforce this distinction, even if the model is unloaded at trigger time.

## 11 Security that supports the assistant's work

### 11.1 Preserve the trust boundaries

The model reads bounded, policy-filtered information and proposes typed actions. Core validates requests and resolves references. The approval service binds authority to exact plans. Engine is the only writer. Parser and conversion tools run isolated on staged inputs. Optional online activity uses a separate constrained route. Elevation is a narrow helper, not a property of the entire app.

The original inherited authenticated IPC and fixed peer matrix should prevent direct Agent-to-Engine authority. Secrets and signing keys stay out of model context. Authority is checked again at execution; a stale plan or forged UI event cannot become permission.

If the model sandbox or process limits fail, the system must not start an unrestricted replacement model process. It can remain useful in explicitly limited deterministic mode, with read-only and approved safe capabilities whose prerequisites are intact. The failed security prerequisite should be visible as degraded status.

### 11.2 Protect private files at every stage

Use explicit read and write scopes, protected folders, content-access rules, safe filename rendering, log redaction and retention controls. Metadata itself can be sensitive. An offline assistant does not automatically have permission to inspect every private document. A search grant should state whether it covers names/metadata, content excerpts or full-file processing.

Document text, filenames, web pages and tool output are untrusted data. Instructions embedded in a PDF or filename cannot change policy, authorize a tool, disable protection or expand scope. The reader context has no planner authority. Online contexts must not inherit private offline context; releasing private material needs the explicit release contract described in the original design.

Keep preference deletion, conversation deletion and recovery-history retention separate. Deleting a chat should not accidentally remove the only information needed to restore a pending transaction. Explain local retention in onboarding.

### 11.3 Define the protection claim honestly

Custodian can protect the user from its own unauthorized or unsafe actions, provide transparent system-security health information, help inspect supported evidence, and integrate with installed security facilities. It must not claim to replace antivirus or guarantee protection against every external threat.

Expose security facts as measured/unknown/unavailable, with evidence source and time. A missing provider is “unknown,” not “safe.” A valid signature is one property, not a guarantee a program is harmless. A filename and a language-model score are not adequate evidence to delete a file.

The current `services/mlhost/mlhost/laya.py` returns approve/delete/quarantine from metadata and model output. The original D-19 specifies calibrated escalate-only decisions. Repair this interface so ML may recommend additional caution or review, but may never lower deterministic risk, grant permission, or directly authorize removal. Build monotonicity tests: enabling ML cannot convert a policy-denied action into allowed.

The original threat model does not promise protection from arbitrary malicious code already running as the same user. State this boundary accurately. Process isolation, least privilege and authenticated plans remain valuable, but claims of tamper-proof protection against equally privileged malware require a different, explicitly reviewed architecture and cannot be inferred from local signatures alone.

### 11.4 Keep offline enforcement independent of prompts

Offline mode must gate all network-capable handoffs, not merely tell the model not to browse. Check broker startup, browser launches, optional provider endpoints, model acquisition and updates. No silent download should repair a missing model during an offline task. The full package must contain verified assets for core operation; a Lite installer must clearly declare its setup network requirement.

Existing online context must not remain active after going offline. Stop the permitted online job and report what stopped. Do not upload file names, logs, embeddings or contents as a convenience fallback. Test the installed product with outbound connectivity blocked, including startup and error paths.

## 12 Proposed technical contracts

These are proposed contracts, not descriptions of types already implemented. Keep one versioned schema shared across Rust, TypeScript and any Python boundary. Validate at both ends and reject unknown or inconsistent authority-bearing fields.

### 12.1 Request and observation

```json
{
  "schema_version": 1,
  "conversation_id": "conversation-reference",
  "task_id": "task-reference",
  "request_id": "request-reference",
  "capability": "files.largest",
  "arguments": {
    "scope_ref": "authorized-local-volumes",
    "count": 1,
    "metric": "logical_bytes",
    "completeness": "live_scan"
  }
}
```

```json
{
  "observation_id": "observation-reference",
  "task_id": "task-reference",
  "status": "partial",
  "observed_from": "2026-10-14T08:35:00Z",
  "observed_to": "2026-10-14T08:37:00Z",
  "scope_ref": "authorized-local-volumes",
  "coverage": {
    "scope_complete": false,
    "access_denied_count": 12,
    "budget_exhausted": true
  },
  "results": [
    {
      "file_ref": "opaque-file-reference",
      "version_ref": "observed-version-reference",
      "logical_bytes": "45204530790"
    }
  ]
}
```

The example is illustrative. Production coverage includes requested, authorized and scanned scopes plus exclusion categories; a single boolean cannot carry all that meaning. Large byte counts travel as decimal strings or another lossless representation across JavaScript boundaries. File references resolve to validated volume/file identities and protected display information inside Core; they are not permission tokens.

### 12.2 Plan and result

A mutation plan contains plan ID/version, task ID, operation list, source identity/version references, destination references, parameters, preconditions, expected effects, inverse strategy, required resources, risk, grant requirements, expiry and a canonical digest. The signed approval binds that digest and the authorized scope. It does not sign only a friendly summary.

```json
{
  "task_id": "task-reference",
  "transaction_id": "transaction-reference",
  "status": "completed",
  "verification": "passed",
  "receipt_ref": "durable-receipt-reference",
  "changed_objects": ["opaque-file-reference"],
  "undo": {
    "state": "available",
    "inverse_ref": "inverse-reference"
  }
}
```

Only Engine/coordinator can emit an authoritative completion receipt. The model cannot create one by returning this JSON. A partial result includes each successful, failed and unattempted item plus recovery status. Completed and verified must not be independently inconsistent fields; enforce allowed combinations in the schema/state machine.

### 12.3 Error handling

Preserve the original error taxonomy: schema, authentication, policy, stale state, space, locked resource, access, timeout, sandbox violation, tamper, unavailable and internal error. Add user-facing explanations and recovery actions without losing machine-readable codes.

| Condition | Correct response |
|---|---|
| Unsupported capability | Explain it is unavailable; optionally provide a clearly labelled handoff |
| Access denied during scan | Return partial coverage or failure as appropriate; do not claim no files exist |
| Source changed after approval | Invalidate the affected plan and inspect/replan; do not reuse stale authority |
| No space for required recovery | Refuse before mutation; offer a scoped alternative without weakening recovery silently |
| Model unavailable | Show model state and retain supported deterministic operations |
| Verification mismatch after change | Record actual state, hold conflicting work and expose recovery-required |
| Transport interrupted | Reconcile by task/transaction ID before retrying a mutation |
| User cancelled | Report safe stopping state, completed effects and remaining work |

Retries require operation-specific idempotency. A read can often be repeated. A move, purge, install or partially completed batch must first reconcile what happened. Never use a generic “retry everything three times” policy for mutations.

## 13 Readiness and performance without misleading claims

### 13.1 Report actual readiness

The current model `check()` verifies file readability and imports the runtime module; it does not fully load and test the model. The server's pong can therefore indicate readiness before actual model initialization succeeds. Separate process alive, protocol authenticated, runtime importable, model loading, model ready, sandbox verified and capabilities ready.

Run a bounded startup health probe appropriate to the capability. Keep deterministic commands available when their own prerequisites are ready. Show loading and actionable failure reasons. Do not declare the whole assistant broken because voice is absent, or fully ready because a child process answered ping.

### 13.2 Optimize after correctness is measurable

The main early wins are metadata-only size queries, bounded top-K memory, fewer repeated walks, deliberate hash staging for duplicates, indexed search with freshness tracking, event throttling, model unload and low-priority background work. Avoid unnecessary GPU assumptions and measure CPU-only startup and inference on a supported baseline device.

Use resource budgets per task and a global scheduler. Yield to foreground user work. Pause/resume large scans where supported, avoid saturating storage, and impose hard limits on parser/conversion processes. Low disk space should reduce safe-place risk, not cause a copy into an already full system drive.

### 13.3 Targets and measurements

The original A24/D-26 targets include under 1% idle CPU, at most 600 MB with models unloaded, fast startup and bounded explanation latency. They are design targets, not measurements from this review. Bundled database/workflow services and multiple runtime processes may make those budgets difficult; measure the actual package before deciding they are satisfied.

Record device CPU/RAM/GPU, OS build, storage type, fixture size, cold/warm state, model hash and enabled services. Report median and tail latency, peak memory, CPU, disk I/O, cancellation latency and time to first useful result. Never compare a warm developer GPU run with a cold CPU-only install as though they were the same test.

Proposed initial responsiveness targets: visible command acknowledgement within 250 ms, a stop request acknowledged within 250 ms, and cancellable scanning stopping forward work within one second on the defined test fixture. These are proposed gates to validate and tune, not claims of current performance. Noninterruptible safe steps need explicit measured bounds and visible stopping state.

## 14 Architecture decisions to settle explicitly

The original README requires synchronized canonical decisions across the three design documents. This roadmap proposes work; it does not enact a replacement architecture. Use a short architecture decision record for deviations, then update all canonical copies when adopted.

| Decision | Existing commitment | Proposal or clarification needed |
|---|---|---|
| AD-01 Process authority | Dedicated Engine only writer; fixed peer matrix | Preserve. Complete the live wiring and remove direct write bypasses |
| AD-02 Storage/workflow | PostgreSQL plus local Temporal with original security and purge constraints | First measure deployment/resource cost. Keep if viable; any simpler replacement needs explicit durability, recovery and purge parity plus a synchronized canonical revision |
| AD-03 Model/framework roster | Named planner/reader/embedding/ML models and Mastra | Evaluate the actual local model/tool combination. Pin assets and document deviations based on task accuracy, latency and isolation evidence, not names alone |
| AD-04 System scope | Strong protected zones and bounded capabilities | Define whole-local-volume metadata discovery independently of write eligibility |
| AD-05 Settings breadth | Sensitive system/security changes forbidden | Publish supported personal settings; broader scope requires a separate product/security decision |
| AD-06 Memory | Allowlisted preferences and separated planner/reader contexts | Specify event-backed conversations, observation freshness and session reset |
| AD-07 ML safety | Escalate-only D-19 | Restore semantics in runtime and training/evaluation contracts |
| AD-08 Parallel work | Durable workflows, low-impact background behavior | Define task state machine, resource locks and scheduler before concurrent writes |

A smaller initial feature surface is compatible with the vision. Silent removal of security boundaries to make a demo work is not. Defer optional online browsing, installations, helper pairing and extra model training until the offline file/task path passes its gates.

## 15 Delivery roadmap: build in this order

Use milestone exit evidence rather than calendar promises. Assign an owner for each milestone; in a small team one person may hold several roles. No milestone is complete because its functions exist or unit tests pass in isolation. Its real desktop journey, failure behavior and relevant recovery gate must pass.

### M0 Establish the truthful baseline and capability boundary

**Purpose:** Stop confusing stubs, unsupported features and model availability with successful work.

**Work:** Reproduce the four reported symptoms in fixtures with local diagnostic IDs. Add a capability/availability inventory. Remove false-success responses from exposed placeholders; disable unsafe dormant operations. Separate model readiness states. Make the UI show structured task status. Record installed/runtime asset status and preserve a baseline of failing scenarios.

**Deliverables:** Capability matrix, symptom traces, readiness contract, defect-to-test register and corrected status behavior.

**Exit:** Unsupported commands never claim execution; failed/declined actions never appear completed; model load failure is distinguishable from an unsupported tool. No disabled unsafe capability remains callable through a different route.

**Owner:** Desktop/runtime engineer with reviewer. **Dependencies:** None. **Audit:** F09, F16, F26–F31, F39–F46. **Original steps:** 31, 55, 79, 83, 88, 91, 93, 100.

### M1 Secure the live execution boundary

**Purpose:** Make the intended isolation and authority rules apply to actual desktop requests.

**Work:** Enforce model isolation without unrestricted fallback; fail closed on required job/IPC setup; establish typed schemas and request/task IDs; centralize grants and online policy; route every mutation proposal through Core/approval/Engine. Remove bypass APIs from reachable dispatch. Protect signing material and replay state. Keep safe deterministic degraded mode.

**Deliverables:** Threat-boundary map, capability checks, process/IPC integration tests and disabled bypass list.

**Exit:** No model can invoke arbitrary writes or network handoffs; sandbox failure cannot launch an unrestricted model; forged, stale and replayed requests fail; unsupported privileges are not silently elevated.

**Owner:** Rust/security engineer. **Dependencies:** M0 inventory. **Audit:** F01–F02, F10–F13, F32, F38. **Original steps:** 9–32, 53, 56, 95–100.

### M2 Ship a correct read-only file assistant

**Purpose:** Solve largest-file, find-file and file-detail requests before broadening writes.

**Work:** Implement explicit scopes, metadata observations, coverage reporting, per-task cancellation and exact result selection. Fix root deduplication. Separate discovery exclusions from mutation protection. Implement singular/top-N queries and size-metric labels. Keep direct commands working offline without model dependence. Add index only after live-scan correctness, with freshness tracking.

**Deliverables:** Working desktop queries across fixture volumes, evidence-backed result cards, partial-result UX and scan benchmarks.

**Exit:** A largest file outside the six known folders is found when in authorized scope. Access denial, budget exhaustion, cancellation and a disconnected drive are reported accurately. Size queries do not read file contents or hydrate offline cloud files. Selection identity persists into follow-up requests.

**Owner:** Core/filesystem engineer and desktop engineer. **Dependencies:** M0 and read boundary from M1. **Audit:** F14–F20, F40–F41. **Original steps:** 40–55, 79, 83, 87–88, 93, 119.

### M3 Make conversation state and task truth coherent

**Purpose:** Eliminate stale operational answers and missing follow-up context.

**Work:** Add coordinator-owned events, conversation/task IDs, structured observations/results, reset/rebuild behavior, direct-route events and approval-outcome feedback. Enforce evidence-derived operational claims. Separate preferences from live facts. Implement reference resolution and truthful tool-budget exhaustion.

**Deliverables:** Session protocol, context projection, memory controls, stale-state regression suite and local diagnostic viewer.

**Exit:** File size changes are reflected in the same chat; direct-command results support “that file”; rejected and completed actions are known to the next turn; restarting/unloading the model cannot invent completion or replay work.

**Owner:** Agent/runtime engineer. **Dependencies:** M0 contracts and M2 observations. **Audit:** F16, F35, F40, F43–F44. **Original steps:** 27–31, 76–85, 88, 93.

### M4 Build the recoverable mutation foundation

**Purpose:** Make one actual file change safe enough to extend.

**Work:** Implement transaction states, prepare-before-write journal, authenticated recovery records, per-resource locks, handle/identity revalidation, destination protection, durable single-use approval and postcondition verification. Fix current cleanup/undo ordering. Make recovery deterministic and usable without AI. Test real same-volume and cross-volume behavior.

**Deliverables:** Single Engine transaction API, verified create/copy/rename fixture journey, recovery console and crash-injection evidence.

**Exit:** A crash at every transaction boundary leaves either unchanged data, a verified completed result or an explicit recoverable state. No successful test may silently skip the second-volume requirement. Edited/tampered recovery records cannot redirect operations. Undo never silently overwrites newer work.

**Owner:** Engine/filesystem engineer with security review. **Dependencies:** M1 authority, M2 identity, M3 outcomes. **Audit:** F03–F11, F21–F24, F36–F38. **Original steps:** 56–62, 69–75.

### M5 Deliver everyday file and settings tasks

**Purpose:** Fulfil commands rather than merely describe how to perform them.

**Work:** Add create folder/text, copy, move, rename, bounded text edit and batch organization adapters. Migrate supported settings to exact-state prepare/apply/read-back/undo. Implement conflict handling, full previews and capability-specific failure messages. Keep unsupported settings as explicit handoffs.

**Deliverables:** Supported-capability catalogue with examples, operation-specific tests and end-to-end command/approve/verify/undo journeys.

**Exit:** Each advertised operation works from the installed desktop on fixtures and has verified success/failure/recovery behavior. The model cannot claim another operation by calling an unrelated tool. Original content and metadata promises are tested for each supported mode.

**Owner:** Adapter engineers and UI engineer. **Dependencies:** M4. **Audit:** F17, F23–F25, F36, F42, F44. **Original steps:** 54–62, 79, 93, 95–103, 111–120 for staged transformations.

### M6 Work alongside the user with durable tasks

**Purpose:** Make background assistance useful without interfering with the person's work.

**Work:** Add task board, bounded queues, independent stop controls, resource locks, foreground-aware scheduling and durable restart reconciliation. Implement Later triggers as re-planned tasks. Keep chat responsive and multiple approvals distinct. Add pause-on-conflict rules for any later UI automation adapter.

**Deliverables:** Concurrent read-task demonstration, conflict-safe mutation scheduler, Later workflow and performance traces.

**Exit:** The person can ask a short question during a scan, stop one task without stopping another, and restart into truthful task states. Two actions on the same object cannot race. Background work respects measured resource limits and does not steal focus.

**Owner:** Coordinator/workflow engineer and UI engineer. **Dependencies:** M3/M4; M5 for mutation demos. **Audit:** F08, F14–F15, F20, F33, F35, F46. **Original steps:** 33–39, 91–94, 102, 116–120, 151, 157.

### M7 Add truthful security health and isolated content tools

**Purpose:** Expand protection and useful processing without expanding model authority.

**Work:** Replace safety placeholders with real measured/unknown provider states. Restore Laya escalate-only semantics and tests. Implement ParserHost and staged conversion tools with sandbox, limits, output validation and Engine finalization. Keep reader and planner contexts separate. Add optional local voice only when commands inherit the same authority rules.

**Deliverables:** Evidence-backed security dashboard, parser/conversion fixture tests, ML monotonicity report and truthful unavailable states.

**Exit:** Missing security evidence is never “safe”; model outputs cannot authorize deletion; a hostile document cannot invoke tools; conversion failure leaves originals intact. Voice cannot raise authority or bypass confirmation.

**Owner:** Security/integration engineer. **Dependencies:** M1/M4/M6. **Audit:** F29–F31, F34, F45. **Original steps:** 63–64, 81–84, 104–115.

### M8 Release a complete offline product

**Purpose:** Ensure the experience works away from the development machine.

**Work:** Package pinned runtimes/models/tools, enforce integrity checks and license requirements, complete local storage/workflow setup, sign/package, produce an SBOM, document upgrade/recovery/uninstall, and test Windows edition and CPU-only variations. Run actual model task evaluation and clean-VM installed tests with network blocked.

**Deliverables:** Reproducible release candidate, offline installer, verification log, supported-device matrix, local recovery guide and honest limitations.

**Exit:** A supported clean PC can install and complete the file query, mutation, verification, undo and stop journeys offline without developer dependencies. All mandatory gates produce real pass/fail status. Unavailable features are not advertised.

**Owner:** Release engineer with full-team sign-off. **Dependencies:** M0–M7 for advertised features; package smoke tests should run earlier at each slice. **Audit:** F26–F28, F33, F39. **Original steps:** 150–170, including real evaluation and final verification.

Optional online browsing, installation automation and helper pairing follow only after the offline release boundary is dependable. Implement original steps 121–149 and helper work as separate gated releases; they are not shortcuts to repairing offline execution.

## 16 Concrete implementation backlog

Each ticket should include a failing acceptance case first, a code owner, source locations, migration impact and completion evidence. The following are proposed tickets, not completed work.

| ID | Deliverable and implementation location | Priority / depends on | Completion evidence |
|---|---|---|---|
| R01 | Inventory live dispatch and disable fake/unsafe capabilities; assistant router, core placeholders | P0 / none | Every exposed capability resolves to a real handler or explicit unavailable state |
| R02 | Separate model readiness; `services/agent/src/llm.ts`, `server.ts`, desktop `agent.rs` | P0 / R01 | Readable corrupt model cannot report model-ready |
| R03 | Fail-closed model sandbox and job setup; desktop agent/process launch | P0 / none | Forced setup failure starts no unrestricted model |
| R04 | Shared request/result schema; `custodian-proto`, agent protocol, UI events | P0 / R01 | Cross-language contract and invalid-state rejection tests |
| R05 | Capability registry; Rust tools and `services/agent/src/tools.ts` | P0 / R04 | No schema/name/availability drift across fast and model routes |
| R06 | Structured UI outcomes; `App.tsx` and message/task types | P0 / R04 | Denial, failure and partial results never render completed |
| R07 | Central network gate; dispatcher/broker/launch paths | P0 / R04 | Offline error paths cannot trigger network handoff |
| R08 | Exact signed plan authority and durable replay handling; approval/IPC/Engine | P0 / R04 | Forged, expired, substituted and replayed plans rejected |
| R09 | Scope types and query cardinality; `assistant/intent.rs` | P1 / R05 | “system,” named drive, selected folder and top-N mean different explicit scopes |
| R10 | Volume/root resolver and read policy; Core plus assistant scope resolution | P1 / R09 | Additional local volume and broader overlapping root retained |
| R11 | Coverage-aware metadata walk; `custodian-core/src/walk.rs` | P1 / R10 | Denied/skipped/budget/cancel reasons present in result |
| R12 | Largest/detail query outputs; `largest.rs`, assistant replies | P1 / R11 | Exact bytes, metric, scope, selection IDs and partial state shown |
| R13 | Per-task cancellation and independent control path; desktop agent/coordinator | P0 / R04 | Stop handled while model/tool is stalled |
| R14 | Event-backed conversation projection; agent and desktop coordinator | P1 / R04 | Direct actions and approval outcomes appear in next-turn context |
| R15 | Observation freshness and invalidation; Core/coordinator | P1 / R12,R14 | External file change cannot be answered from old fact as current |
| R16 | Session reset/rebuild and reference resolution; protocol/UI/agent | P1 / R14 | “Move it” resolves exact selected object across model unload |
| R17 | Evidence-derived factual replies and tool-budget behavior; agent/replies | P0 / R04,R15 | Irrelevant tool call cannot validate success claim |
| R18 | Transaction state machine and prepare-before-write journal; Engine/journal | P0 / R08 | Crash-injection evidence at each durable boundary |
| R19 | Authenticated recovery metadata and integrity-before-write; recovery/undo/purge | P0 / R18 | Tamper/missing record holds operation safely |
| R20 | Resource locks and object/destination revalidation; Engine/fsops | P0 / R18 | Race and path-substitution fixtures cannot redirect writes |
| R21 | Same/cross-volume verification and metadata contract; fsops/undo | P0 / R20 | Real two-volume round-trip suite with fidelity checks |
| R22 | Migrate current cleanup/settings off bypass routes; assistant/actions/settings | P0 / R18–R21 | All live mutations use prepared transactions |
| R23 | Create/copy/rename/move adapters; registry/Engine | P1 / R22 | Installed UI task plus undo for each supported adapter |
| R24 | Bounded text-edit and organize adapters; Engine/staging/UI | P1 / R23 | Changed-source and occupied-destination cases preserve user data |
| R25 | Exact settings snapshot/read-back/conflict-aware restore | P1 / R22 | Raw original state restored; later user changes not overwritten |
| R26 | Indexed metadata and reconciliation; indexer/store | P2 / R11,R15 | Watcher gap invalidates completeness and triggers reconciliation |
| R27 | Durable task coordinator and restart recovery; selected workflow/store runtime | P1 / R18,R14 | Restart reconciles work without duplicate effects |
| R28 | Task board, multiple approvals and scoped stop; desktop UI | P1 / R13,R27 | Two independent tasks remain independently controllable |
| R29 | Resource scheduler and user-activity yielding | P1 / R27 | Measured CPU/I/O/cancel budgets on supported hardware |
| R30 | Deferred plans/Later with fresh revalidation | P2 / R27,R08 | Trigger cannot execute stale references or expired authority |
| R31 | Real security-health providers and unknown states | P1 / R04 | Unavailable provider cannot produce safe verdict |
| R32 | Laya escalate-only interface and evaluation | P1 / R31 | ML cannot reduce risk or grant action authority |
| R33 | ParserHost and isolated staged transformations | P2 / R03,R18 | Hostile/malformed input cannot escape or alter originals |
| R34 | Full offline asset/installer manifest and readiness smoke test | P0 / R02 | Clean VM needs no developer checkout or silent download |
| R35 | Evaluation harness nonzero failure and release gates | P0 / none | Deliberately failing case fails CI/release command |
| R36 | Installed end-to-end suite and benchmark report | P1 / advertised capabilities | Real command-to-effect checks, no mandatory skipped gates |
| R37 | Canonical design decisions/document sync | P1 / each accepted deviation | Master/architecture/build plan agree with implemented boundary |
| R38 | Privacy retention, preference controls and recovery-safe chat deletion | P2 / R14,R27 | Clear-chat cannot break recovery; memory cannot remove protection |

P0 means release-blocking safety/truth/foundation work, P1 means essential product capability, and P2 means useful expansion after prerequisites. These labels do not imply every P0 ticket must finish before independent read-only work can start. Do not parallelize conflicting code changes or release unsafe capabilities simply because another milestone is still in progress.

## 17 Map the original 170-step build plan to the recovery programme

This table maps the original plan to the new delivery sequence. It is not a step-by-step certification of implementation. Existing code should be reused where its live integration and verification pass; do not rewrite good primitives merely because the product is incomplete.

| Original steps | Original area | New programme and verification focus |
|---|---|---|
| 1–11 | Foundations and platform/security spikes | M0/M1/M8: reproducible baseline, real sandbox/firewall/platform checks |
| 12–32 | ACLs, AppContainers, jobs, IPC, schemas, health | M1: enforce in the actual launch/dispatch path; M0 readiness distinction |
| 33–39 | PostgreSQL, Temporal, store roles and supervision | AD-02 then M6/M8: deployment/resource spike, real task persistence |
| 40–55 | File identity, state, scanner, kernel and queries | M2/M4: complete scope/coverage and execution preconditions |
| 56–62 | Signed execution, journal, safe place and undo | M4: mandatory foundation for every advertised mutation |
| 63–64 | Parser and excerpts | M7: isolated reader data, no implicit planner authority |
| 65–72 | Specialized deletion/offload/cloud/purge/anchor | Hold unsafe/unwired paths; M4 gates before re-enabling supported subset |
| 73–75 | Roundtrip/race/recovery gates | M4 release blockers; genuine two-volume/crash/tamper evidence |
| 76–88 | Agent, models, tools, grammar, evidence, search | M0/M3: honest readiness, structured observations, integrated context |
| 89–103 | Desktop, approvals, health and complete offline loop | M0/M2/M5/M6: visible verified outcomes and task control |
| 104–110 | ML, Laya, forecasting and voice | M7: optional capability only after measured safety gates |
| 111–120 | Sandboxed tools and durable file/Later/scan workflows | M5–M7: staged outputs, coordinator, re-planning and recovery |
| 121–137 | Optional online mode and broker | Later release: preserve offline privacy and context separation |
| 138–149 | InstallLab, elevation and installation saga | Later release: no generic elevation shortcut for current task failures |
| 150–158 | Onboarding, recovery, uninstall, helper, performance, accessibility | M6/M8: backup/recovery truth, usable controls, measured runtime |
| 159–165 | Training and evaluation | R35 early; model training only after task-failure attribution |
| 166–170 | Installer, signing, clean VM, security and document gate | M8 plus package smoke tests throughout development |

The critical offline path is: trustworthy result contracts → secure dispatch and file identity → accurate reads → coherent task context → recoverable writes → useful adapters → background coordination → complete offline packaging. Security and installer smoke tests begin early and run throughout; they are not postponed until the last milestone.

## 18 Acceptance tests specifically for the reported problems

These 40 additional proposed cases complement A01–A55 in the audit. They are specifications, not a claim that these tests were executed. Use generated fixtures, disposable directories, controlled settings adapters and dedicated VMs. Never run destructive acceptance scenarios against personal folders. Each case records request, route, observations, task state, actual filesystem/setting effect and recovery result.

| ID | Scenario | Required result |
|---|---|---|
| B01 | Ask file size, externally enlarge the file, ask again in the same chat | New size from fresh evidence; old observation not promoted to current fact |
| B02 | Ask a deterministic file query, then “move the second one” | Exact visible result reference resolves; no basename guessing |
| B03 | Decline an action card, then ask whether it happened | Rejected outcome known to UI and model; no change and no success claim |
| B04 | Approve and finish an action, then ask about it | Response uses verified receipt and distinguishes history from current state |
| B05 | Unload/restart model after a result, then refer to that result | Required context rebuilt or clear clarification; no invented context |
| B06 | Start a new conversation | Previous conversational references not silently reused; recovery history retained |
| B07 | Ask a live-state question after tool budget is exhausted | Incomplete status or scheduled bounded continuation; no fabricated current fact |
| B08 | Model calls an unrelated tool and claims a file moved | Claim rejected; receipt absent and UI not completed |
| B09 | Fixture's largest file lies on second authorized local volume | Correct file returned with both volume scopes shown |
| B10 | Largest file lies outside the six known personal folders | Found under appropriate whole-volume read policy |
| B11 | Largest file lies in a dependency/dot directory | Included if query policy allows; otherwise exclusion clearly reported |
| B12 | One directory denies metadata enumeration | Partial/coverage condition visible; no exhaustive whole-PC assertion |
| B13 | Scan reaches budget before visiting known larger file | Partial result and continuation option; no false maximum claim |
| B14 | Cancel scan mid-traversal | Per-task cancelled state, partial evidence labelled and bounded stop response |
| B15 | Request one largest file, then top ten | Correct cardinality and deterministic tie policy |
| B16 | Sparse/compressed file and hard-linked fixture | Logical/allocated/reclaimable distinctions respected; no misleading free-space claim |
| B17 | Cloud placeholder in scope, network blocked | No silent hydration; metadata/exclusion treatment disclosed |
| B18 | New large file created after indexing or watcher gap injected | Index invalidated/reconciled or clearly labelled non-exhaustive |
| B19 | File vanishes or changes during scan | Accurate interval/partial/stale behavior; no crash or stale action target |
| B20 | Source path replaced after approval | Execution rejects changed identity/version; replacement remains untouched |
| B21 | Destination already contains different user file | No silent overwrite; clear conflict and revised plan if requested |
| B22 | Crash before/after every mutation journal boundary | Restart reconciles to unchanged, verified or explicit recoverable state |
| B23 | Cross-volume move on actual separate test volumes | Content and promised metadata verified; undo tested; no conditional silent skip |
| B24 | User edits output after operation, then requests undo | Conflict preserves new work; no automatic destructive restore |
| B25 | No space for recovery/staging | Refusal before mutation, truthful error and intact source |
| B26 | Supported settings change API returns but read-back differs | No success receipt; failure/recovery result recorded |
| B27 | Original setting absent/type-specific; later user changes it | Exact inverse and conflict-aware undo verified |
| B28 | Unsupported setting request | Explicit unsupported/handoff result; never “changed” |
| B29 | Scan running while user requests short status and stop | Chat/control responsive; no wait for scan or next model frame to accept stop |
| B30 | Two mutations target same file or destination parent | Safe locking/ordering; no race or double execution |
| B31 | Two independent tasks with one cancelled | Only selected task stops; other task and UI remain coherent |
| B32 | Multiple simultaneous approval requests | Separate task-bound cards; no overwritten plan or wrong approval target |
| B33 | Restart during background task and trigger Later | Reconcile first; replan current state; no duplicate effect/expired grant reuse |
| B34 | Model sandbox/job setup deliberately fails | No unrestricted model fallback; truthful degraded deterministic mode |
| B35 | Model file readable but corrupt/incompatible | Process-alive distinct from model-ready; actionable unavailable state |
| B36 | Offline mode with missing asset and network-capable request | No silent download/broker bypass; core installed capabilities still work |
| B37 | Malicious instruction in filename/document | Remains data; cannot grant authority, call tools or alter protection |
| B38 | Laya suggests approval/deletion against deterministic restriction | Cannot lower risk or authorize action; escalates/abstains only |
| B39 | Security provider unavailable or signature-only evidence | Unknown/limited evidence shown; no “system/file is safe” fiction |
| B40 | Install on supported clean CPU-only VM without developer tools, network blocked | Advertised offline command, actual effect, verification, undo and stop all work |

Run the exact user phrasing and reasonable typos/paraphrases, including “largest file in my system,” “what is its size,” “move it,” “change this setting,” and “work on this while I continue.” Also test low-confidence language, ambiguous references and multilingual requests only if those languages are advertised. The expected action matters more than the style of the model's prose.

## 19 Testing, release gates and proof of progress

Maintain layers of evidence: pure function tests for policy/algorithms; contract tests for schemas; integration tests for actual process boundaries and transactions; adversarial fixtures for races/tamper/prompt injection; UI end-to-end tests for user journeys; installed offline tests; and real local-model evaluations. A mocked tool reply is useful for conversational routing but does not prove the file operation works.

The earlier audit recorded passing automated checks after environment-related test issues were resolved. It also recorded ignored/conditional hardware tests and historical model evaluation limitations. Keep those qualifications. Documentation changes do not require repeating the entire application suite, and this extension does not claim a new full test run.

Fix the evaluation harness so failed cases fail the command/CI gate. Record tested model/runtime hashes and actual tool execution mode. Separate deterministic routing accuracy, model tool selection, policy refusal accuracy, operation success, evidence correctness and undo success. A single “86% accuracy” figure from canned tool responses cannot establish overall assistant reliability.

Use a release dashboard with capability, implemented, integrated, fixture-tested, installed-tested, recovery-tested, offline-tested and remaining limitations. Do not call a capability verified if its only test was skipped, mocked or run against a different route than the desktop. Stop release on unauthorized write/network access, false success, data-loss recovery failure, isolation fallback or mandatory gate omission.

For every completed milestone retain: build identifier, test command/report, fixture description, device profile, observed task receipt, relevant screenshots or UI transcript, and unresolved limitations. Store private data out of shared evidence; use synthetic names/content.

## 20 Practical first implementation batch

The first batch should make the existing product tell the truth and solve a real read-only task while the write foundation is repaired.

1. Add the capability inventory and expose unavailable operations honestly. Disable fake-success and unsafe placeholder routes.
2. Introduce structured outcomes and stop using successful chat transport or approval as a success signal.
3. Fix readiness and isolation fallback. A model is either safely ready, loading or unavailable.
4. Add task/conversation IDs and record deterministic results plus approval outcomes in one event stream.
5. Correct “my system” scope, preserve requested top-N, and return coverage-aware largest-file results with exact sizes and selectable references.
6. Require fresh observation for mutable PC facts and verified receipts for completion claims.
7. Make cancellation independent and per-task so long work can stop promptly.
8. Repair prepare-before-write recovery, authenticated records and execution revalidation before introducing general move/edit operations.

Demonstrate the resulting slice with one controlled scenario: find the largest fixture file across two authorized locations, change it externally and correctly re-query it, select it in a follow-up command, then execute a supported recoverable operation only after the mutation foundation passes. The demo must include denied access, cancellation and restart recovery, not only a happy path.

## 21 Definition of the product being ready

Custodian is ready for a bounded offline release when a new user can install it on a supported PC, understand its access scope, and complete every advertised task without Internet or developer setup. The displayed result must agree with the measured system state and durable task receipt. The user must be able to stop work, inspect history and safely undo supported operations. Private data and protected system areas must remain within the declared policy even when model output is wrong.

Readiness does not require implementing every optional idea in the original master document. It does require every exposed capability to have a truthful availability state, a tested execution path, clear limits and the appropriate security/recovery guarantees. The product should grow by adding verified capabilities to that foundation.

## 22 Glossary for readers joining the project

| Term | Plain-language meaning |
|---|---|
| Capability | A specific supported task, such as moving a file or reading a setting |
| Adapter | Code that implements one capability using a controlled Windows/file interface |
| Observation | A measured fact from a particular source and time |
| Receipt | Durable evidence of what a task actually did and verified |
| Scope | The files, folders, drives or settings a request covers |
| Grant | A bounded permission from the user/policy for a task or capability |
| Plan | Exact proposed operations and their conditions before execution |
| Preconditions | Facts that must still be true before an operation may proceed |
| File identity | A stable reference to an object, stronger than its displayed filename alone |
| Journal | Durable transaction records used to recover after interruption |
| Idempotency | Retrying/reconciling a request without causing its effects twice |
| Revalidation | Checking again that the approved object and conditions have not changed |
| Sandbox | An enforced boundary limiting a process's access and resources |
| Fail closed | Refuse the unsafe operation when a required protection is unavailable |
| Partial result | Useful work was done, but some requested scope/items remain unverified |
| Escalate-only ML | A model can add caution; it cannot remove restrictions or grant authority |
| Vertical slice | A complete user journey from request through real execution and verification |

