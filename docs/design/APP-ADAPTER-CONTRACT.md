# Application adapter contract

This document freezes the first Stage 4 boundary before any real application
automation is enabled. It deliberately describes a small, typed surface; it is
not permission to add general mouse, keyboard, shell, or screenshot control.

Review implementation, 18 October: native request/identity/grant contracts and
fixture checks exist. No real application executor, hard-timeout worker,
persistent private-app UI, or accepted editor/spreadsheet adapter is enabled.

## Supported initial effects

An adapter may expose only these operations for a qualified application:

1. `open_bound_document`
2. `inspect_bound_document`
3. `edit_bound_range`
4. `save_bound_document`
5. `close_document_normally`
6. `verify_saved_output`

Every operation must be represented as a typed coordinator `Step` with a native
capability name, explicit dependencies, bounded inputs, a timeout, and an
effect receipt for every write or application action.

## Required target identity

The adapter must bind all of the following before observation or input:

- executable identity, including the normalized executable path;
- process ID and a process-creation identity, so a reused PID is rejected;
- top-level window identity and generation;
- document identity and observed version/hash where available;
- control or range identity for the exact text/cells being changed;
- the current privacy-policy revision.

An operation must fail closed as `StaleWorld` when any identity changes, the
window disappears, the document changes, focus is lost for a focus-dependent
operation, or the privacy revision is no longer current.

Document identity is a native opaque ID distinct from its potentially duplicate
display name. Requests and receipts carry that ID; grants also bind its version.
The future host must resolve file-backed IDs under current file/privacy policy,
and qualify mixed-tab/private-document behavior before any content observation.
The native process probe measures executable and creation time through one
handle without enumerating titles. It does not establish a window generation,
document identity or continuing process lease by itself.

## Authority rules

- Model output can select a registered capability but cannot create a
  capability, grant, path, window handle, process identity, or permission.
- Native code binds names and references to measured observations.
- A saved plan is evidence, not authority.
- New application effects do not inherit file-copy or file-backup authority.
- Unknown dialogs, secure prompts, UAC, macros, external links, uploads, and
  executable launches stop the task rather than accepting a guessed action.
- A normal close request must never force-terminate an application.

`ApplicationPrivacyPolicy::allows` is privacy eligibility, not effect authority.
The checked execution gate requires a native, non-deserializable grant scoped
to its issuing authority, app identity, document ID/version and permitted actions.
Edit scope also binds the exact range and replacement digest. Grants expire and
are revocable; privacy revision changes invalidate earlier grants. Native
remeasurement and authority are checked again before the action. A returned
unverified/mismatched receipt is not accepted as success.

The initial range contract uses half-open UTF-8 byte offsets. A qualified adapter
must check character boundaries against the exact observed version and convert
to its API's coordinate system before input; it must not pass those offsets
directly to a UTF-16 UI control. Shape validation alone cannot establish this.

The host may issue fresh native step bindings under existing suitable standing
consent after independently verifying its preceding effects. These tokens do
not imply prompting the user for every edit/save step. Models cannot mint them.

`execute_checked` supplies cooperative cancellation/deadline checks, not a hard
timeout or process sandbox. A hung UIA/API call still requires the supervised
worker. If a returned effect receipt is followed by cancellation or revocation,
the gate retains the receipt as recovery evidence and redacts it from Debug.
That evidence is not a durable journal or permission to replay/undo. The host
must checkpoint before effects and independently reconcile actual app/files.

The registered inspect operation is a Read step, saving/editing/opening/closing
are App steps, and saved-output verification is Verify. A planner cannot label
an edit as a Read/Verify step to evade effect rules. Normal multiline Unicode
text is permitted within bounds; NUL/unsafe control characters remain refused.

## Initial synthetic acceptance fixture

Before enabling a real adapter, tests must include:

- two documents with the same title;
- one unrelated unsaved document;
- a private application/window canary;
- a changed source document;
- a replaced destination;
- focus loss and window movement;
- a stale/reused process identity;
- an unknown modal dialog;
- an application that stops responding;
- a crash after the application action and before publication of the receipt.

The fixture must prove that no edit reaches an unrelated document and that an
ambiguous or interrupted action is reported as uncertain/recovery-required,
never as successful.

The machine-readable future real-app cases are in
[app-acceptance-fixtures.json](../EVIDENCE/stage-4/app-acceptance-fixtures.json).
They are definitions, not passing results. Existing coordinator/app fixtures
remain explicitly mock; native grant tests that write temporary files are not
evidence of actual editor control.

## Implementation order

1. Add native identity and policy checks.
2. Add the adapter worker with bounded calls and cancellation.
3. Qualify the text-editor adapter.
4. Connect it through `TaskCoordinator` and receipts.
5. Add the planner capability only after native tests pass.
6. Qualify a spreadsheet adapter separately; it must not reuse text-editor
   assumptions.

Stage 4 is not accepted until the optimized desktop build passes the real
application walkthrough and the adversarial cases above. Unit tests alone do
not establish application-control acceptance.
