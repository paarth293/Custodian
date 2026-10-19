# Stage 4 real application implementation

This report supersedes the capability status in the earlier foundation, worker
and privacy increment reports. Those reports and their artifacts remain historical
evidence. Later Stage 5 and Stage 6 core increments are recorded separately in
their execution records; neither stage is accepted. The current cross-stage
verification is in [the 19 October audit](VERIFICATION-AUDIT-2026-10-19.md).

## Implemented scope

The native desktop accepts explicit replace-text and set-cells requests, prepares
one immutable native plan, and asks for one task approval. Each workflow stages
a copy beside the selected permitted destination, uses a new dedicated application
instance/profile, verifies saved output independently, closes normally, publishes
a new file through the existing file executor and checks publication. Originals
are preserved. Receipt-backed undo preserves subsequently changed outputs.

VS Code 1.140.0 uses a pinned local extension, authenticated current-user named
pipe, actual document/editor references and process/window generation bindings.
Only bounded UTF-8 .txt files without BOM/NUL, exact unique replacement selections
and 64-KiB results are qualified. An active-tab switch invalidates the binding
before further document content is returned. No global keys or reflected commands
are exposed.

Excel 16.0.20430.20146 uses a new hidden instance through a supervised native
worker. Pre-existing instances are never selected. The supported subset is inert,
single-sheet .xlsx workbooks within A1:P16, General/text formats, bounded literal
values and fixed SUM formulas. Inputs are bounded ZIP/XML-admitted before Office
opens them; macros, external relationships and unsupported content are refused.
Formula-looking text remains literal. Full approved results are predicted and
compared with actual state and independently parsed saved bytes.

The Excel worker uses authenticated bounded IPC, a one-process Job with 512-MiB
memory and 25-percent CPU limits, per-call deadlines and no queued replay after
failure. Startup is bounded to 60 seconds, ordinary Excel requests to 15 seconds,
close to 30 seconds and session lifetime to 30 minutes. Editor startup is bounded
to 60 seconds, ordinary requests to 10 seconds and close to 30 seconds. Stop and
authority checks run during transport waits. An in-flight external application
effect may finish in its owned staging area; stopping does not mean it was undone.

Application journals use append-only DPAPI records, sequence/hash-chain checks,
exclusive ownership, bounded directory enumeration, 256 records, 512-KiB individual
records and an 8-MiB total budget. Prepared scope and intended artifact fingerprints
are durable before effects. Restart inspection compares actual files without
restoring grants, replaying actions, overwriting changed data or publishing output.
The task board exposes read-only recovery inspection after restart.

## Security and limits

Private app/document admission precedes document reads. Persistent policy revision,
Stop, exact native identity and document state are checked around actions. Native
modal disabling invalidates a window lease. Unknown prompts never authorize a
default response, privilege change, upload or macro. General UIA content reading,
window-title inventory, screenshots and visual fallback remain disabled.

The trusted application worker is resource-contained, not a filesystem/network
sandbox. Excel and VS Code are not network-isolated by these adapters. The local
model's existing AppContainer boundary is separate. App exclusions do not replace
private file-path rules. DPAPI and generation tags do not protect against a
compromised same-user host or valid historical-blob rollback.

Uncertain or revoked sessions can remain open for manual normal close. The host
does not force-close unrelated or newly private work. Arbitrary applications,
arbitrary Office formulas, multilingual app planning and general desktop operation
are not qualified. Laya, model weights and the earlier agent modules are unchanged.

## Acceptance record

Frozen A401–A412 expectations and their lock are preserved unchanged. Working
records are under `target/stage-4-completion`; final evidence must distinguish
native fixtures, desktop execution, package verification and installation.

Real native editor tests have passed stale-content refusal, private-tab switching,
altered native target claims, Stop, revocation around measurement/Save, an actual
crash after saved effect before receipt, duplicate titles, focus/window movement
and a native modal. The actual crash test inspects twice without replay and
preserves a subsequent edit. Real Excel tests verify unrelated unsaved workbook
preservation, a distant out-of-scope sentinel, recalculation, saved reopen and undo.

The development desktop passed one real workflow for each app and recovery
inspection after restart. Optimized repeated acceptance, complete final regression,
current installer and disposable offline installation remain pending until their
actual results are recorded. A corrected rectangular SAFEARRAY fixture now passes;
the earlier invalid-index failure is retained. Failed performance, fixture and
concurrent-build attempts remain part of the evidence and are not counted as
successful acceptance runs.

No claim of complete Stage 4 acceptance is made by this implementation report.
