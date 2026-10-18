# Offline assistant: execution and acceptance plan

Planning revision: 17 October 2026. This document changes no application code.
It turns the original pasted vision and subsequent offline-first decisions into
an ordered delivery plan. It supplements the master plan; it does not declare
its requirements implemented or replace the existing evidence.

Progress, 18 October 2026: Stage 0 is complete after the
[runtime baseline repair](STAGE-0-RUNTIME-REPAIR-2026-10-17.md), including two
fresh 23-check desktop passes. [Stage 1](STAGE-1-AUTHORITY-AND-RECOVERY-2026-10-18.md)
is complete for its declared authority/recovery boundary: 361 native tests,
131 JavaScript tests, four actual process-kill boundaries and a 27-check
optimized-desktop acceptance run pass. [Stage 2](STAGE-2-TASK-COORDINATOR-2026-10-18.md)
is complete for its bounded coordinator foundation: 379 native tests, 134
JavaScript tests and two 28-check desktop runs pass, including the optimized
artifact. [Stage 3](STAGE-3-LANGUAGE-AND-PLANNING-2026-10-18.md)
has passed its frozen file-language evaluation (24 supported and 12 negative
cases), real model-to-file execution, sandboxed-runtime checks and a 29-check
optimized desktop run with exit 0, including cleanup. Two earlier production
cleanup failures and the subsequent fixture-driver fix remain recorded.
This increment covers the registered file-copy combinations; broad references,
exclusions, multilingual execution and general app planning remain open.
Stage 4 has not started. Full-product,
compiler-hardening and installation acceptance remain open.

## 1. What completion means

A person can give an ordinary-language offline goal, let the assistant perform
permitted work across local folders and supported applications, see verified
results, interrupt execution, and recover without losing newer work.

Whole-PC reach means arbitrary permitted local paths and supported apps, subject
to OS access and private zones. It does not mean administrator authority or
universal compatibility with every application. Examples must exercise reusable
tools; adding a new phrasing or combining existing tools must not require a new
hard-coded scenario handler.

Two release gates keep scope visible:

- **Demonstration candidate:** stages 0–6 plus the applicable security,
  installation and usability checks in stages 11–12. It remains a partial product.
- **Agreed Windows offline release:** every stage below, every mandatory row in
  the capability register, and stage 12's acceptance suite. Only this gate can
  justify completion of the agreed Windows offline scope.

Online browsing/accounts/sending/cloud inference remain deferred by the owner's
decision. macOS/Linux remain later platform releases with their own native
adapters and acceptance. These exclusions must appear in product and submission
claims. Broad offline settings, content, development and software management
must not silently become future scope just because they are difficult.

No release can promise every imaginable task, every third-party app, or zero
possibility of harm. Exact supported operations and boundaries must be explicit.
There is no assumed deadline, staffing level or guarantee of winning a hackathon.

## 2. Baseline to preserve

The workflow report records 342 Rust tests passing, four ignored environment
tests, 131 JavaScript tests, and two actual 23-check desktop walkthroughs. These
are evidence for particular behaviors, not a completion percentage.

Reuse local pinned Qwen inference, the model sandbox, native file policy,
private paths, sealed receipts, verified undo, bounded collection execution,
standing permissions and cancellation. Preserve tests and failed evaluations.

Current gaps include general planning, actual app observation/action, content
extraction and editing, comprehensive authority enforcement, true crash drills,
multi-instance coordination and installed-product acceptance. Existing settings,
reminder and app code must be audited: a historical implementation claim does
not establish that it is reachable and verified in the current UI.

Laya is an experimental semantic evaluator with failed activation targets. It
currently grants no authority. Its intended production role is decided in stage
3 from evidence, not its name or presence in the repository.

## 3. Ordered execution

### Stage 0 — Reconcile requirements and establish a working baseline

1. Create one capability register covering every original domain. Give each
   operation an ID, status, code owner/component, dependency, acceptance task,
   evidence location and release gate.
2. Trace each claimed capability from user input through policy, execution and
   verification. Label implemented-but-unreachable, simulated and stubbed paths.
3. Record current source/runtime hashes and preserve the dirty working tree;
   establish a reproducible baseline without discarding existing changes.
4. Define supported Windows versions, reference hardware, local filesystems,
   initial app versions and file formats. Record missing tooling explicitly.
5. Freeze representative tasks and expected outputs before implementation.

**Exit:** every original requirement has a visible disposition. No “supported”
row rests solely on a mocked tool response. Existing proven workflows still run.

### Stage 1 — Complete the shared authority and recovery foundation

1. Define one typed capability contract: inputs, resources, effects, permission
   requirements, preconditions, verifier, cancellation and recovery behavior.
2. Route exposed file, settings, app and later script operations through that
   contract. Retire bypasses; do not restore paused tools without migration.
3. Extend private-zone enforcement to app/window content, accessibility trees,
   screenshots, extracted content, logs and indexes. If screen protection cannot
   be enforced, disable that capture route rather than capture then redact.
4. Generalize standing grants by operation, resource and consequence. Show their
   scope in plain language. Capture grant revisions before queuing work.
5. Serialize mutations across processes or enforce a single authoritative
   broker instance. Revocation must affect queued and active workflows.
6. Add real process-kill tests before mutation, mid-copy, after child receipt
   persistence and before parent completion. Reconcile on restart without replay.
7. Exercise disk-full, unavailable destinations, corrupt records, changed files,
   path replacement and stale permissions. Preserve ambiguous output for review.

**Exit:** all designated authority/privacy/recovery cases pass using real
fixtures. No model can fabricate consent. Stop prevents later actions and takes
effect at a documented safe boundary for any action already committing.

### Stage 2 — Build a general task coordinator

1. Introduce task state with the user goal, constraints, known facts, scoped
   grants, proposed steps, dependencies, receipts and explicit completion tests.
2. Support typed discovery/read/write/app/verify steps and bounded branching;
   outputs of earlier steps can feed later steps without exposing raw execution.
3. Add conflict-aware resource locking, task-specific Stop, pause at safe
   boundaries, bounded retries, checkpoints and truthful partial completion.
4. Classify failures: retryable observation, stale world state, missing authority,
   unavailable capability and unrecoverable failure. Replan only from new facts.
5. Put progress, permission needs, completed effects and recovery choices into
   one understandable task view. Prevent late results from an old conversation
   initiating work in a new one.

**Exit:** deterministic fixture plans compose file and fake adapter steps through
the same coordinator; real file compositions preserve existing safety evidence.
The mock adapter is labelled mock until stage 4 supplies real applications.

### Stage 3 — Make language understanding and model planning useful

1. Replace scenario-specific wording as the main entry path with structured
   intent extraction: goal, references, constraints and desired result.
2. Resolve paths/app identities from measured observations. Ask only when a
   consequential ambiguity cannot be resolved from the conversation.
3. Let Qwen select and compose registered capabilities, including dependencies
   and completion predicates. Native code validates every proposed step.
4. Keep retrieved content separate from user instructions. Minimize and label
   model-visible data; private content never enters the model.
5. Add bounded repair from verifier feedback and limits on actions, time, retries
   and resource use. Missing capability must produce an honest limitation.
6. Evaluate Laya as a narrow advisory checker of intent/effect consistency or
   uncertainty. Compare Qwen-only and Qwen-plus-Laya on frozen cases, including
   latency and dangerous misses. Use existing activation criteria or document
   stricter replacements before testing. Failed candidates stay experimental;
   deterministic enforcement remains mandatory in either configuration.
7. Test varied wording, multilingual ambiguity, references such as “those”,
   exclusions, conflicting instructions and impossible tasks. Do not confuse
   valid JSON or a correct plan with successful execution.

**Exit:** frozen intent/plan evaluation meets declared targets; actual execution
evidence grows as adapters arrive. New combinations of registered tools work
without adding one handler for each sample sentence. Laya's role and result are
explicitly recorded, including a decision to leave it inactive if warranted.

### Stage 4 — Implement real local application control

1. Add observation of eligible windows and accessible controls with private-app
   checks before observation. Identify the correct app, window and document.
2. Prefer app APIs or accessibility actions; verify state after every meaningful
   action. Use text editor and spreadsheet applications as the first two adapters,
   selecting installed versions during stage 0.
3. Implement open, navigate, enter/edit content, save, switch windows and normal
   close. Handle unsaved changes and modal dialogs without discarding data.
4. Add a constrained visual fallback only where observation privacy, target
   identity and allowed effects can be enforced. Uncertain controls trigger
   clarification; raw coordinate clicking is not an authorization boundary.
5. Classify transitions that expose terminals, privileged dialogs, uploads or
   other consequential controls and enforce their policy before input.

**Exit:** actual editor and spreadsheet workflows survive focus changes, moved
windows, unexpected dialogs and an unrelated open document. Saved results are
checked from application state and file contents. Private windows remain unseen.

### Stage 5 — Complete file and storage work

1. Generalize folder creation, batch rename/move/copy and organization with an
   exact manifest, conflict handling, stable resource identities and recovery.
2. Add bounded archive creation/extraction; defend against traversal, links,
   oversized expansion and overwrite. Reopen and verify archive contents.
3. Implement versioned local backup, manifest/retention rules and restore to a
   chosen location. Perform restore drills; copy undo is not a backup product.
4. Implement governed recycle/trash deletion and measured cleanup. Show exact
   targets and consequences; preserve recovery. Permanent deletion is distinct
   and must not be implied by ordinary organization permission.
5. Cover permitted roots on multiple local volumes, long/Unicode paths, locked
   files, removable-drive loss and duplicate names. Specify unsupported volume
   and link behavior rather than guessing.

**Exit:** real files and full directory manifests match expectations after
operations and restore. Every destructive capability has explicit authorization
and tested recovery limitations. No silent omission or partial success claim.

### Stage 6 — Add grounded content and complete multi-app work

1. Implement isolated extraction for plain text/Markdown, CSV, DOCX, PDF and XLSX,
   with limits on file size, parser time and memory. Never execute embedded code.
2. Generate and edit these formats plus presentations using format-aware tools.
   Keep source references for factual reports and preserve unrelated content.
3. Verify artifacts by reopening: document content/layout, spreadsheet values
   and formulas, slide structure/rendering, PDF text/pages and encoding.
4. Add local image resize/crop/convert, OCR and audio/video inspection/conversion
   as separately declared capabilities with fidelity and resource checks.
   Generative media and transcription require their own evaluated local models.
5. Complete three varied task families: prepare a verified handoff archive and
   inventory; create/update a grounded report in an editor; transform a dataset,
   produce a checked spreadsheet and open it for review.
6. Interrupt, modify inputs and change app state during those tasks. Demonstrate
   safe adaptation and partial-result reporting.

**Exit:** changed wording, filenames, locations and inputs succeed through shared
capabilities. At least two real applications are exercised. The three examples
are acceptance samples, not the limit of what combinations are supported.

### Stage 7 — Implement personal settings and local diagnostics

1. Audit existing six per-user settings and verify their UI paths end to end.
2. Add audio, supported display settings, power preferences, default-app flows,
   Bluetooth/network-adapter controls, peripherals and startup-app configuration
   through specific Windows adapters where reliable observation/restore exists.
3. Diagnose using measured local facts. Separate diagnosis from proposed repair;
   never equate a symptom with a proven cause.
4. Preview consequential changes, save previous state and verify the result.
   Display changes need a tested timeout/revert flow; unsupported OEM hardware
   must report unavailable. Account for disruption to the user's active work.
5. Manage only identified processes with consequence checks; closing an app
   must respect unsaved work. Security/boot/recovery changes remain blocked.

**Exit:** each advertised setting is read, changed, checked and restored on its
supported hardware/OS matrix. No successful registry write alone counts as a
verified visible setting change. Offline network configuration does not enable
online assistant services.

### Stage 8 — Add restricted development and local software management

1. Provide project-scoped code edits, patches and local Git inspection/changes.
   Remote operations stay deferred. Treat hooks and build definitions as code.
2. Build a restricted command runner with structured arguments, process-tree
   containment, resource/time limits and scoped filesystem access. A command
   allowlist alone is insufficient for arbitrary scripts, hooks or plugins.
3. Run builds/tests/debugging only against authorized projects and targets with
   available offline dependencies. Record outputs and verify requested results.
4. Support identified local installers/packages: inspect publisher and requested
   effects, stage changes, request consequence-specific consent and verify the
   installed version. Elevation is exact and temporary, never model-controlled.
5. Implement supported local update/uninstall paths and explicit recovery limits.
   Unknown installers are unsupported until an execution strategy is validated;
   a digital signature does not prove harmlessness.

**Exit:** real project edit/build/test and supported install/update/uninstall
journeys pass in disposable environments, including hostile hooks, failed builds,
partial installs and unexpected child processes. No general privileged shell.

### Stage 9 — Finish local routines, reminders and preferences

1. Audit and connect persistent local tasks/reminders; implement restart and
   missed-trigger behavior without duplicates.
2. Save reusable routines as capability plans with visible scopes, expiry where
   appropriate, budgets and revocation. Materially changed effects need new consent.
3. Add local draft text/EML, ICS events and permitted contact-file processing.
   Offline drafts are not sent messages or synchronized calendar events.
4. Make proactive automation explicitly opt-in and quiet unless it has a useful
   result, failure or decision requiring attention.

**Exit:** routines execute once as intended across restart, revoke correctly,
and never turn an old grant into permission for a new class of action.

### Stage 10 — Complete interaction and resource behavior

1. Make keyboard, screen-reader, focus, progress, Stop, recovery and permission
   controls work through actual assistive interfaces.
2. Add evaluated multilingual task execution. Local voice is optional for initial
   Windows release; if enabled, test transcription uncertainty, cancellation,
   permissions and microphone privacy rather than assuming voice equals consent.
3. Profile CPU-only and available GPU configurations, cold/warm startup, memory,
   responsiveness and long tasks. Display real readiness and measured progress.
4. Bound concurrency and indexing. Avoid repeated model loading and wasted prompt
   preparation; check that background activity does not impede user control.

**Exit:** beginner walkthroughs complete without developer assistance, and
reference-hardware responsiveness meets the budgets fixed in stage 0. Ordinary
safe authorized work does not require repeated per-step approvals.

### Stage 11 — Adversarial and failure acceptance across all routes

Run targeted checks with each stage, then a full suite here:

- Instructions hidden in files, extracted text, app labels and tool responses.
- Private paths/apps/screens, credential stores, renamed targets, reparse points,
  stale handles, changed permissions and multiple app instances.
- Forged/replayed approvals, model-supplied paths, command arguments, rogue
  children, unsupported UI transitions and resource exhaustion.
- Stop/revoke during discovery, planning and execution; abrupt death, full disks,
  unavailable drives, corrupt journals and user edits before recovery.
- Unexpected network attempts during otherwise offline tasks.

**Exit:** every mandatory safety case passes; no unresolved high-impact defect
in a release path. A failure blocks that capability's release instead of being
averaged away by many passing benign tests. This is a defined threat-model
assessment, not a guarantee against privileged hostile software or all attacks.

### Stage 12 — Installed-product and independent acceptance

1. Test clean offline installation in a disposable clean Windows environment,
   prerequisites, first launch, ordinary-user execution, restart and uninstall.
   Resolve missing WebView2 behavior explicitly; test upgrade preservation and
   compatible recovery records. Install/uninstall must preserve unrelated data.
2. Validate package/runtime/model hashes, update behavior and signing strategy.
   Until signing is available, label development packages accurately.
3. Run a frozen minimum 60-task end-to-end suite covering every mandatory
   capability and varied multi-app compositions, at least twice with fresh state.
   Proposed target: at least 95% verified task success, with no failed mandatory
   showcase workflow or safety gate. Record every failure; do not round up.
4. Keep development cases separate from acceptance cases. Have someone who did
   not tune the system supply at least 20 additional supported-scope tasks.
   Freeze expected outcomes before testing; retire exposed holdouts for later
   evaluation. If no independent evaluator is available, mark this gate pending.
5. Complete the repository's required five repeated original walkthroughs and
   the new multi-app/failure cases. Reuse valid evidence without relabelling
   different tests as equivalent.
6. Observe at least five consenting users on representative tasks. Record task
   success, intervention count, time versus manual work and confusing behavior.
7. Freeze an accepted build, capability matrix and known limitations. Prepare a
   live demonstration with changed inputs, an interruption/recovery example and
   an explicit unsupported request. Confirm actual event rules when available.

**Exit:** all required evidence belongs to the delivered version or a justified
unchanged component. Remaining optional features are labelled. Mandatory gaps
cannot be removed from the register merely to declare completion.

## 4. Permission behavior: autonomy without continual interruptions

Reads and reversible in-scope work execute under visible standing permissions.
One goal can authorize a multi-step task when its effects are sufficiently clear.
The assistant asks again for material scope/consequence changes, ambiguity that
could affect the wrong resource, external disclosure, elevation or destructive
actions outside the current authorization. Critical protection/boot/private-zone
overrides are not delegated to the model. The same rules apply to API, UI and
script routes; a different executor must not create an authority bypass.

Approval count is an acceptance metric. Reducing prompts is achieved by precise
reusable grants and reliable execution, never by silently broadening authority.

## 5. How each implementation slice will be delivered

1. Select a requirement ID and write its user-visible completion predicate.
2. Implement its policy, executor, verifier, recovery and real UI integration.
3. Run focused unit/integration tests and real application/OS tests where needed.
4. Exercise relevant failure and authority cases before exposing it.
5. Record source/build identifiers, raw outcomes, failed cases and limitations.
6. Update the register and honesty report. Move to the next dependency only when
   the gate passes, or explicitly record an unresolved blocker and work on an
   independent requirement. Never count the blocked gate as complete.

Progress updates should state: completed requirements; demonstrated behavior;
remaining gaps; next requirement. Test counts alone are not progress toward
scope. Avoid repeated percentage or “10/10” ratings without a defined rubric.

## 6. Immediate execution order

Start with stage 0's capability register and baseline. Then close stage 1's
cross-process authority and real crash-recovery gaps. Build the shared task
coordinator and general planning interface, add real app control, and complete
the first file–content–app workflow. Continue through every remaining offline
domain in stages 7–10, then finish cross-route and installed acceptance.

Do not rebuild the proven file foundation from scratch. Do not keep adding
collection variants while app/content execution remains absent. Do not call a
demonstration candidate the complete offline assistant.

## References

- Original vision: user attachment `43545615-b687-420a-a99d-af05ddf6d46b/Pasted text.txt`.
- [Master plan](OFFLINE-HACKATHON-MASTER-PLAN.md).
- [Current workflow evidence](MODEL-PLANNED-WORKFLOWS-2026-10-17.md).
- [Acceptance contract](ACCEPTANCE.md).
- [Current implementation limitations](../HONESTY.md).
- [Laya evaluation](LAYA-IMPROVEMENT-VERIFICATION-2026-10-17.md).
