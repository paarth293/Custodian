# Stages 4–6: detailed work and acceptance plan

Planning only, 18 October 2026. No capability is implemented or accepted by this
document. The existing execution plan, capability register and frozen acceptance
expectations remain authoritative. Numerical targets below are proposed new gates
to freeze before implementation; they do not replace any existing gate.

## Starting point and delivery rules

Stage 3 accepted a bounded file-language increment: six collection combinations,
measured location bindings, real local Qwen inference, native authorization,
receipt-backed execution and undo. Its model is not a general desktop operator.
The coordinator has typed app steps, but the demonstrated app adapter is mock.
App content observation is disabled; the current reader returns unavailable.
The historical installer is still Stage 0. Laya remains inactive.

The Stage 0 inventory recorded Notepad launcher 10.0.26100.8457 and Excel
16.0.20430.20146. These are candidates, not qualified adapters. Recheck actual
running versions and local readiness; launcher identity is not editor identity.

Complete one numbered work package and its acceptance evidence before expanding
the next. Finish Stage 4 before Stage 5 and Stage 5 before Stage 6. Working
subsets may be reported separately, but do not mark an entire stage complete
while a mandatory part is missing. Preserve the accepted Stage 3 bundle and
source snapshot throughout. Do not change frozen expectations to fit results.

Every new capability needs a typed input/output contract, measured resource
bindings, effect classification, native permission check, bounded execution,
independent verifier, receipt/checkpoint and explicit recovery limitations.
Extend the existing coordinator and authority rather than creating an app or
content executor that bypasses them. Model text and saved plans never grant
permission. New effect classes do not inherit old collection authority.

Whole-system reach remains arbitrary permitted local paths and supported apps,
subject to OS access, private zones and qualified adapters. Do not hard-code
Documents/Downloads as the operating boundary. Unsupported apps/volumes remain
explicitly unsupported until measured. No arbitrary shell or permanent admin.

## Stage 4: real application control

### 4.0 — Freeze the adapter contract and test environment

- Inspect actual installed editor/spreadsheet versions without reading personal
  documents. Determine whether a dedicated automation instance/window is possible.
  Measure supported API/accessibility behavior and normal save/close paths.
- Define versioned adapters and capabilities such as open a bound document,
  inspect an eligible document, set selected text/range, save to a bound path,
  inspect a known dialog, activate a bound window and request normal close.
- Define stable bindings: process ID plus creation identity and executable;
  window identity/generation; document identity/version; control identity and
  allowed action. Reacquire after stale handles; never trust a reused PID/HWND.
- Freeze synthetic editor/workbook fixtures, private canaries, unrelated unsaved
  work, required outputs, failure cases and support matrix before implementation.
- Baseline packaging debt: prepare a current-version runtime/package and an
  offline installation smoke-test plan in a disposable profile/VM. Do not execute
  an installer against the user's normal environment as a test.

Deliverable: adapter specification, installed-version probe and frozen cases.
Gate: both candidate apps support an enforceable target/save strategy. If one
cannot, document the blocker and qualify an alternative explicitly; do not count
the mock or a launch-only route as a substitute.

### 4.1 — Enforce private-app policy before observation

- Add persistent private-app/session/window exclusions, alongside private paths.
  Use minimal native process/window identity to decide eligibility before reading
  titles, accessibility text, document content, thumbnails or pixels.
- Limit observation to the bound document/control subtree. Never inventory every
  window's content first and filter private material afterwards.
- Prefer a dedicated, known document session. Shared-process/tabbed apps require
  a trustworthy current-document binding before content access. If an app cannot
  establish it, do not attach or observe its content. A process allowlist cannot
  establish document privacy within a mixed session.
- Recheck policy revision, target identity and current document before every
  observation/action. A user's tab/document switch invalidates the old binding.
  Generic check-then-capture cannot guarantee privacy against arbitrary UI races;
  keep such capture disabled unless a supported adapter contains that boundary.
- Carry privacy into task cards, model context, error reports and diagnostic
  logs. Do not log a private canary or its title/path as proof of refusal.
- Revocation cancels queued work and invalidates model-visible observations.

Deliverable: app privacy policy and enforcement at all exposed observation/input
entry points. Gate: private and unknown document contexts remain unread, including
when the user switches a previously eligible window to private work.

### 4.2 — Build the bounded native app executor

- Isolate accessibility/API calls in a supervised worker so an unresponsive app
  cannot hang the desktop. Bound calls, tree depth/node count, returned text,
  memory and total task duration; record chosen limits before the acceptance run.
- Prefer direct app APIs or control-scoped accessibility patterns. Do not use
  unqualified ActiveWorkbook/ActiveDocument references or global SendKeys.
- Re-observe around meaningful actions and verify postconditions. A successful
  invocation, focus change or keystroke call is not evidence of saved content.
- Use a foreground interaction lease for actions that require focus. User input
  or changed focus invalidates pending input. Yield to the user rather than
  repeatedly stealing focus. Actions supported through background APIs need no
  unnecessary focus change.
- Treat app text as untrusted data. UI instructions cannot authorize a terminal,
  security dialog, upload, executable launch, macro or permission change.
- Classify dialogs before acting. Unknown modal dialogs stop the action chain;
  never press a guessed default button. Do not automate UAC/secure desktop.

Deliverable: real adapter execution through existing App steps, checkpoints,
resource locks and native authorization. Gate: stale targets, focus loss, denied
controls and hung calls fail safely without input reaching unrelated work.

### 4.3 — Qualify the text-editor adapter

- Create/open a known local text file in the selected editor; identify the actual
  document window; edit an explicitly selected range or create new content.
- Preserve unrelated text and declared newline/encoding behavior. Detect source
  changes before replacing an existing file; retain a recoverable prior version
  when editing is authorized.
- Save to an authorized destination; verify document saved state and independently
  reopen the bytes. Support normal close and recognized unsaved-change dialogs.
- Preserve another unsaved editor window/tab. Do not close or overwrite it merely
  because its title matches. Undo must retain a user's later edits.

Gate: actual open/edit/save/reopen/close succeeds with moved/resized windows,
focus changes, duplicate titles, a changed source and an unrelated unsaved note.

### 4.4 — Qualify the spreadsheet adapter

- Bind one workbook and worksheet explicitly; qualify range reads/writes,
  navigation, supported arithmetic formulas, save, reopen and normal close.
- Work only on task-owned or explicitly authorized workbooks/ranges. Recheck
  document version and protect unrelated workbooks, sheets and cells.
- Disable automatic macro execution and external-link updates on task opens;
  refuse unsupported active content and protected/privileged contexts. Verify
  effective behavior on the installed version before enabling the adapter.
- Treat imported formula-like strings as data by default. Do not execute DDE,
  external references, network formulas or arbitrary user-supplied automation.
- Verify saved values, supported formulas and actual calculated results. The
  adapter's initial fixture operations are not Stage 6's general XLSX generator.

Gate: real workbook changes persist after reopen; an unrelated workbook and
out-of-scope cells remain identical; macro/link prompts never become consent.

### 4.5 — Connect language, Stop, pause and recovery

- Expand the typed planner registry with only qualified app operations. Bind
  names/references to native app/document observations; ask only for consequential
  ambiguity. Keep existing file-language and native tool routes working.
- Allow ordinary granted edit/save steps without per-step approval. Require new
  authority for broader destinations, unrelated documents, overwrites or effects
  outside the current grant. Keep consequence and consent separate from model
  confidence. A model cannot certify that its next action is harmless.
- Extend task cards with the selected app/document, verified steps and recovery
  choices. Stop blocks later actions; report a bounded in-flight operation as
  stopping until it is quiescent. Do not claim cancellation has undone a save.
- After crash/restart, reconcile actual document/files and receipts. Never replay
  a saved app action automatically. Pause at a safe boundary, not mid-keystroke.

### 4.6 — Decide on visual fallback

Use an adapter-local visual fallback only if pre-capture privacy, exact target
identity and allowed effects can be enforced and tested. A global screenshot
followed by redaction is not sufficient. Otherwise keep fallback disabled and
record that decision. Visual fallback is not required to accept an API/UIA
adapter that covers its declared workflows; arbitrary visual desktop control
must not be implied.

### 4.7 — Stage 4 acceptance and delivery

Freeze normal and adversarial scenarios before development. Proposed minimum:
five consecutive successful executions of each editor and spreadsheet core
workflow, plus all mandatory privacy, stale-target, modal, focus-loss, revoked
grant, unsaved-work, crash and changed-output cases. Repeat through the optimized
desktop, not only an external test harness. Existing tests remain required.
Record first-attempt success, retries, intervention, latency and cleanup failures.

Deliver executable/runtime/package identities, real file comparisons, permitted
app-state evidence, worker-containment checks, user-facing limits and selected
source restore verification. Test a newly built package offline in a disposable
environment; if that environment is unavailable, installation stays unaccepted.
Stage completion requires both real app adapters and all mandatory behavioral
gates. Full security/signing acceptance remains separately tracked.

## Stage 5: complete file and storage work

### 5.0 — Introduce a shared batch manifest and selection contract

- Represent every planned source/destination, file identity, size, hash where
  needed, operation, conflict rule, exclusion and expected final state.
- Resolve named files and conversational collections from measured results.
  Add exact name/path/type/size exclusions through structured predicates. Content
  exclusions remain unavailable until Stage 6 extraction supports them.
- Bind private-path exclusions before traversal; refuse a parent mutation that
  would implicitly move/delete private descendants. Avoid silent subset success.
- Define limits per new capability. Do not silently increase the existing
  20-file/100-MiB collection grant by introducing a larger batch executor.

Gate: preview, execution and verifier agree on the same exact manifest; ambiguous
references and unsupported predicates cause clarification without substitution.

### 5.1 — Generalize create/copy/move/rename/organize

- Compose existing primitives into bounded directory creation and batch actions.
  Implement deterministic conflict choices: stop, explicitly skip, or select a
  new name. Replacing existing data needs distinct authority and recovery.
- Handle case-insensitive collisions, duplicate basenames and rename cycles.
  Reserve all affected resources before mutation and recheck identities/grants.
- Same-volume moves and cross-volume moves require different recovery protocols.
  Cross-volume work copies, verifies and records the destination before any
  separately authorized source removal. Do not claim whole-batch atomicity.
- Journal each effect before/after execution. On interruption, distinguish
  verified complete, unstarted and ambiguous items. Compensation only reverses
  receipt-owned unchanged outputs; preserve newer user changes.

Gate: full before/after directory manifests and file bytes match expectations;
cross-volume failure never loses the only verified copy.

### 5.2 — Add bounded archive creation and extraction

- Start with an explicitly declared ZIP subset. Creation uses an approved exact
  manifest, writes a new output and reopens it to verify entry names and bytes.
- Extraction preflights all entries before writing into a new staging directory.
  Reject traversal, absolute/drive/UNC names, alternate data streams, links,
  reserved names, case/Unicode-normalization collisions and protected/private
  destinations. Recheck actual paths/identities at write time; string prefix
  checks alone do not contain an extraction race.
- Proposed initial additional limits: 1,000 entries, 512 MiB expanded bytes and
  100:1 expansion ratio, bounded memory/time and capacity reserve. Freeze tested
  values before implementation. These are archive-specific ceilings, not an
  expansion of old copy permission. Enforce actual streamed bytes as well as
  headers; nested archives are not recursively extracted automatically.
- Reject encryption or formats whose safe verification is unsupported. Do not
  execute extracted files. Publish final output only after verification; retain
  truthful recovery information for interrupted publication.

Gate: normal round-trip bytes match; every hostile archive fixture is contained;
no entry outside the bound destination is created or replaced.

### 5.3 — Build versioned local backup

- Define a backup set, exact source selection, owned destination, immutable
  version identity, content/integrity manifest and retention configuration.
- Store a new verified version without replacing older versions. Detect changed
  or locked sources; do not label a mixed-time copy a consistent snapshot.
  Snapshot-dependent app backups remain unsupported unless separately qualified.
- Interrupted versions remain incomplete and cannot be selected as healthy.
  Retention deletes only verified backup-owned versions under separate authority;
  it never deletes arbitrary neighboring files or the only healthy version.
- State metadata preservation, encryption and threat-model limits explicitly.
  Same-machine local backup does not establish ransomware-proof protection.

### 5.4 — Implement and drill restore

- Restore a selected verified version to a chosen permitted new location by
  default. Reconstruct and compare the complete manifest, not just file counts.
- Original-location replacement needs separate conflict/overwrite permission,
  preservation of current data and a tested recovery protocol.
- Drill corrupted manifests/data, missing versions, interrupted backup/restore,
  edited destinations and drive loss. Record unrecoverable cases honestly.

Gate for 5.3–5.4: a real independent restore reproduces the expected tree and
bytes; invalid/incomplete versions are rejected; retention cannot erase the
only verified recovery point.

### 5.5 — Add governed recycle deletion and measured cleanup

- Add a distinct recycle authority with exact targets and consequences. Copy or
  organization permission does not grant deletion. Normal language must not turn
  into permanent deletion, Recycle Bin emptying or system cleanup.
- Check supported Windows provider/volume recovery behavior before mutation.
  If recycle/recovery cannot be established, refuse; do not fall back to unlink.
- Identify cleanup candidates using measured rules and an explainable manifest.
  Age/size alone is not proof a file is disposable. Recheck changes before action.
- Record native recycle identity/receipts and recovery limits. Verify originals
  are recycled, then perform actual restore drills. Permanent deletion remains
  a separate disabled capability unless separately specified and accepted.

Gate: default refusal without authority; authorized synthetic targets restore;
private/system/unrelated files and newer user edits remain intact.

### 5.6 — Qualify filesystem boundaries and disruptions

Run on permitted C:/D: roots and arbitrary custom locations, Unicode/long paths,
duplicate names, case collisions, read-only/locked files, full disks and lost
removable volumes. Test revocation and conflicting tasks during each mutation.
Declare supported filesystems/volume types. Reparse points, hardlinks, cloud
placeholders, network volumes and unsupported metadata stay refused until their
own identity/privacy/recovery strategy is accepted. No automatic elevation.

### 5.7 — Stage 5 acceptance and delivery

Freeze actual tree/byte/metadata expectations and run every mutation together
with its verification and recovery case. Include process-kill tests at new
journal/publication boundaries, not only synthetic journal edits. Verify the
existing frozen cases, including B03/B06, against their unchanged expectations.

A ZIP that exists, an undo button or copied backup files are insufficient. Require
full manifests, actual archive reopen, independent backup restore, recycle restore,
source preservation and honest partial outcomes. Update planner capability
contracts, UI permissions, package smoke tests, evidence/hashes and source backup.
Stage 5 is incomplete if archives, backup/restore or governed recycle work is
still missing, even when batch copy works.

## Stage 6: grounded content and complete multi-app work

### 6.0 — Define isolated content contracts and provenance

- Replace unavailable extraction with typed local-file reader contracts; do not
  relabel the existing web-marker reader as a functioning document parser.
- Open only policy-approved native file bindings. Run parsers/renderers in
  contained workers with read-only inputs, task-owned scratch/output, no network,
  no shell, bounded decoded data and supervised child processes.
- Proposed initial parser ceilings: 64 MiB input, 512 MiB worker memory and
  60 seconds per extraction. Add per-format page/cell/object limits; benchmark
  and freeze the supported subset before acceptance. Archive limits also apply
  to packaged office formats. A timeout returns unavailable/partial, not success.
- Represent source hash/version, page/paragraph/sheet/cell/range, extracted facts,
  extraction warnings and confidence. Verify permissions before model exposure.
- Pass only necessary permitted excerpts to the existing sandboxed model through
  authenticated IPC. Preserve instruction/data separation. Document text cannot
  issue new instructions or broaden file/network authority. Keep private content
  out of prompts, logs, thumbnails and diagnostics, not only final answers.
- Bundle and pin actual workers, format dependencies, fonts and models needed
  offline; development-machine availability does not establish packaged support.

### 6.1 — Qualify text, Markdown and CSV first

- Handle declared encodings, newlines, quoted/multiline CSV fields, missing
  values, types, delimiters and malformed rows without silent data loss.
- Reopen generated outputs and compare structured values to independent expected
  fixtures. Preserve formula-like CSV strings as data unless a validated formula
  operation is explicitly authorized.
- Build reusable read/select/transform/write/verify capabilities, not a custom
  handler for each report or dataset sentence.

### 6.2 — Add DOCX, PDF and XLSX extraction

- DOCX: paragraphs, tables and declared comments/headers support, with location
  references; unsupported tracked-change/embedded-object behavior explicit.
- PDF: pages, extracted text and declared table support. Detect image-only or
  incomplete extraction; OCR requires its separately qualified capability.
- XLSX: worksheets, types, formulas and cached/calculated-value distinctions,
  merged/range boundaries and warnings for unsupported workbook features.
- Never execute macros, embedded executables, external links or parser-provided
  commands. Corrupt/encrypted/oversized documents fail within bounds.

Gate: each advertised format matches frozen facts and source locations; private
and hostile content cannot acquire tool authority; unsupported content is visible.

### 6.3 — Generate and safely edit artifacts

- Generate/edit text/Markdown/CSV, DOCX, XLSX, PDF and PPTX through format-aware
  writers with a declared feature subset. Use new outputs/task-owned staging by
  default. Editing existing artifacts needs precise scope and version checks.
- Preserve unrelated paragraphs/sheets/slides and supported formatting. Save a
  recoverable prior version before authorized replacement. If the writer cannot
  preserve a requested feature, refuse that edit rather than silently strip it.
- Reports retain links to source versions and locations. Verify deterministic
  facts/numbers independently. Conflicting or missing sources are reported;
  source citations alone do not prove every natural-language claim correct.
- Spreadsheets use native transformations for arithmetic, a restricted formula
  grammar and a qualified calculation engine. Compare formulas and evaluated
  values; a workbook XML cache is not proof recalculation worked.
- Presentations require actual editable slides, layout checks and slide renders.
  PDF outputs require text/page/encoding checks and rendered-page inspection.

### 6.4 — Build independent format verifiers

Reopen all outputs through separate read/verification paths where practical.
Check text/structure and required facts, document/table layout, spreadsheet
formulas and totals, slide structure/rendering, PDF pages and encoding. Inspect
renders for clipping, overflow and unreadable text; machine checks alone are not
a claim of flawless visual layout. Include false-success tests: blank but valid
files, missing pages, stale totals, truncated text and wrong output destinations.

### 6.5 — Connect grounded language and reusable multi-app programs

- Extend interpretation beyond Stage 3's quoted-folder copy selector to typed
  content goals, measured conversational references, supported exclusions,
  artifact outputs and app review steps. Keep unresolved ambiguity explicit.
- Let Qwen choose/combine registered discovery, read, transform, write, app and
  verify capabilities with typed dependencies. Native code validates schemas,
  resource bindings, grants, budgets and completion facts before execution.
- Keep the planner, content generator and authority distinct even if they share
  one local model. Reference hardware/context limits require chunking and source
  selection; never silently omit material and claim a complete-source report.
- Maintain step facts/receipts and derived provenance so the assistant can explain
  where a result came from and which parts actually completed.

### 6.6 — Implement bounded observation and replanning

- Detect source version changes, missing/changed app targets, known dialogs,
  revoked authority, ambiguous outputs and failed verification separately.
- Proposed initial repair bound: at most two re-observation/replan cycles within
  the total task budget. Freeze actual deadlines before evaluation. Retain goal
  and constraints; do not widen grants or remove difficult requirements.
- Recompute affected pure read/transform steps from fresh inputs. For writes/app
  effects, first reconcile real outputs and receipts. Never repeat an ambiguous
  effect because the model requested a retry. Append plan revisions/checkpoints
  rather than erasing evidence of the first attempt.
- Ask only when an unresolved consequence requires the user's choice. Recognized
  harmless recovery can run under the current grant; unrecognized dialogs stop.
- Stop prevents further effects; pause retains safe state and locks. Restart
  remains review-first, without automatic effect replay. Show verified partial
  results and safe recovery options when the whole goal cannot be completed.

### 6.7 — Qualify local media as separate capabilities

- Images: resize/crop/convert with declared formats, orientation, color, alpha,
  metadata policy and decompressed-pixel limits. Reopen and verify dimensions,
  fidelity expectations and output integrity; preserve originals.
- OCR: use an explicitly packaged offline engine/model, declared languages and
  page/image bounds. Retain region/page provenance and uncertainty; poor OCR
  cannot silently become a trusted financial or factual value.
- Audio/video: bounded metadata inspection and explicit conversion parameters,
  supported local codecs, streams, duration and output validation. Supervise
  subprocess trees and cancellation; never treat a filename change as conversion.
- Generative media and transcription need separately evaluated local models,
  licensing/runtime review and hardware budgets. They remain unavailable unless
  those gates pass. A text model or a byte-marker stub is not such a capability.

Deliver image, OCR and declared audio/video support or mark the relevant Stage 6
part incomplete. The three showcase families below do not remove this work.

### 6.8 — Prove three reusable task families

1. Handoff: select permitted files with exact exclusions, create inventory and
   archive, reopen/verify every entry, and show the final local outputs.
2. Grounded report: read declared sources, compose a fact-linked report, open it
   in the real editor, make a supported revision, save and independently reopen.
3. Dataset: read CSV, apply typed transformations, create a checked spreadsheet,
   calculate totals, open it in the qualified spreadsheet app and preserve the
   unrelated workbook. Frozen B09 totals remain 100, 300 and 400.

Vary names, local volumes, folder locations, wording, input content and order.
Change a source, move/focus a window, introduce a modal, revoke permission and
Stop mid-task. Reuse the same capability registry; do not add one handler per
demo sentence. Formats not covered by these three families still need their own
real execution and verification tests.

### 6.9 — Stage 6 acceptance and delivery

Preserve every existing frozen expectation. Author and freeze additional
format/media, injection, privacy, grounding, repair, stale-source and recovery
cases before implementation. Include separately supplied cases not used for
prompt tuning. Label author-written regressions honestly.

Proposed broader reliability gate: at least 24/25 additional supported end-to-end
tasks succeed without manual repair; record every intervention/retry/failure and
time. All mandatory showcase, verification, privacy, authority and recovery cases
must pass regardless of that aggregate score. No unauthorized effects, private
canary disclosure, unrelated-data damage or false completion is acceptable in
the declared acceptance suite; passing it is not a universal safety guarantee.

Deliver real outputs, source references, format/render verification, actual app
state, receipts, cancellation/recovery evidence, optimized runtime/package tests,
offline disposable installation smoke results and source restore evidence.
If any mandatory format, media or app capability is missing, accept only the
named subset and keep Stage 6 open. Independent full-product evaluation remains
later work; stages 4–6 cannot establish universal automation or a hackathon win.

## Order and review at each boundary

Sequence: 4.0 environment/contracts → 4.1 privacy → 4.2 executor → 4.3 editor →
4.4 spreadsheet → 4.5 coordinator/language → 4.6 fallback decision → 4.7 acceptance;
then 5.0 manifests → 5.1 batches → 5.2 archives → 5.3 backups → 5.4 restore →
5.5 recycle → 5.6 boundaries → 5.7 acceptance; then 6.0 containment/provenance →
6.1 simple formats → 6.2 office/PDF readers → 6.3 writers → 6.4 verifiers →
6.5 language/composition → 6.6 repair → 6.7 media → 6.8 varied workflows →
6.9 acceptance. Work-package boundaries permit useful validation without calling
an unfinished stage complete.

At each review record: requested behavior, implemented contract, tested versions,
real outcomes, native authority/privacy checks, resource bounds, failure/recovery
behavior, package status, retained failures and exact artifact/source identities.
Run relevant checks and existing regressions; use the real model and production
app where behavior depends on them. Freeze app code before optimized acceptance.
Update capability status only when its stated evidence exists. Laya stays
inactive unless its separate unchanged activation gates actually pass.

## Existing references

- [Execution plan](OFFLINE-EXECUTION-PLAN-2026-10-17.md)
- [Capability register](../reference/CAPABILITY-REGISTER.md)
- [Frozen future acceptance cases](../reference/STAGE-0-ACCEPTANCE-FIXTURES.json)
- [Stage 3 evidence and limitations](../verification/STAGE-3-LANGUAGE-AND-PLANNING-2026-10-18.md)
- [Stage 0 environment and app candidates](../verification/STAGE-0-BASELINE-2026-10-17.md)
