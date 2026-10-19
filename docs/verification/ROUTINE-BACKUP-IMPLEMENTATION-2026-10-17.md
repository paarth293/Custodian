# Verified routine backup — 17 October 2026

This increment introduces one complete native workflow: discover the largest
permitted ordinary file in the requested local scope, copy it to an existing
destination, verify both contents and persist a receipt that supports undo after
restart. It is a small single-file backup workflow, not a general backup system.
The complete offline operator remains unfinished.

## Use

Open **Routine permissions**, enable automatic verified backup and save. This
standing permission defaults off. Then ask:

> Back up the largest file in "C:\Projects" to "D:\Backups"

Both locations must already exist. Replace the source with `"all drives"` to
discover files across accessible local volumes. Scope is not restricted to the
personal-folder shortcuts. Stop cancels discovery; a copy already running
finishes safely. Ask `undo` to remove the last verified copy while preserving
the original. Other operations retain their existing approval behavior.

The quoted goal is parsed by native code directly from the person's message.
It uses deterministic selection and execution; the model does not plan or
authorize this workflow. Laya remains experimental and disconnected from
desktop authority. Neither a chat request nor a model tool can change the
standing permission.

## Authority and integrity

- The permission is stored with Windows DPAPI and a required-state marker.
  Corrupt state or missing previously saved state fails closed. A revision binds
  the request to its original grant; revoking and re-enabling cannot revive it.
- Privacy checks, native protected-path checks, local-volume requirements and
  ordinary-file exclusions still apply. Known cloud-sync folder names are also
  refused. This name filter does not identify every possible sync application.
- The largest eligible candidate must fit the limit. The workflow does not
  silently substitute a smaller candidate. Limits are 100 MiB per request, at
  most 5% of destination free space, leaving at least 1 GiB free. The executor
  checks capacity again immediately before journaling the mutation.
- Copy preparation and execution hold source and ancestor Windows handles with
  restrictive sharing. Opened objects are checked for reparse/cloud attributes;
  the source must have one hard link. Ordinary source writes, renames and ancestor
  replacement are refused while guarded. Destination creation never replaces
  an existing file, and the created output stays guarded through receipt saving.
- The existing file-task executor saves a sealed prepared receipt before creating
  the output, hashes source and copy, and records completion only after successful
  verification. Undo verifies the recorded identity and content before removal.
  Failed or partial copies retain recovery evidence; automatic crash recovery is
  not claimed.

These controls do not protect against arbitrary same-user malicious code,
privileged attackers or writable memory mappings. Existing move/rename paths
have not received the new copy guard. Third-party applications may react to a
new file and are not network-isolated by this feature. File discovery is a scan,
not an atomic snapshot of every file on the PC; inaccessible and excluded files
are outside its result.

## Verification

Source checks passed on Windows:

- [332 Rust tests passed](../EVIDENCE/routine-backup/rust-tests-2026-10-17.txt),
  with four existing opt-in environment tests ignored. This includes actual
  DPAPI persistence, stale-grant and changed-source refusal, privacy exclusions,
  capacity/size refusal, restart/undo, and safe in-flight revocation without
  blocking capture of unrelated requests.
- [122 JavaScript tests passed](../EVIDENCE/routine-backup/javascript-tests-2026-10-17.txt).
  [Lint and type checking](../EVIDENCE/routine-backup/javascript-lint-types-2026-10-17.txt)
  and the [production frontend build](../EVIDENCE/routine-backup/frontend-build-2026-10-17.txt)
  also passed.
- [All-target Rust Clippy passed with warnings denied](../EVIDENCE/routine-backup/rust-clippy-2026-10-17.txt).
- The Windows copy-guard regression actually attempts source writes/renames,
  ancestor renaming and output overwriting while handles are held. Each is
  refused, and the copied bytes and existing destination remain intact.

The initial sandboxed JavaScript runner failed to access its temporary files;
the normal-profile rerun above passed. This was not an application test failure.

The [intermediate desktop preflight](../EVIDENCE/routine-backup/desktop-preflight-2026-10-17.txt)
passed 13 checks using actual Windows accessibility controls and fresh marked
C:/D: fixtures. It covers private-location refusal, measured search, declined
and approved move, restart/undo, default-off routine authority, automatic backup,
existing-output refusal, persisted grant/copy undo, independent answers and Stop
during whole-PC discovery, revocation during discovery, and offline refusal.
Its [executable record](../EVIDENCE/routine-backup/preflight-artifact-2026-10-17.json)
labels it as an intermediate build; it does not certify the final package.

An earlier invalid preflight used the older copied executable after a wrong
source filename. It timed out at the default-off routine assertion and is
excluded, not counted as a successful walkthrough. The corrected launch checks
the executable hash before testing. Original logs and fixture evidence remain
under `target/routine-ui-preflight-01.log` and the marked preflight-01 directory.

Two fresh walkthroughs of the final production executable each passed all 13
checks: [run 1](../EVIDENCE/routine-backup/desktop-final-01-2026-10-17.txt) and
[run 2](../EVIDENCE/routine-backup/desktop-final-02-2026-10-17.txt). Both use the
same final executable and separate marked fixtures. The original remains on C:;
copies are created on D: and removed only by verified undo. The private canary
is larger than the selected public file and stays unchanged and unreported.
The whole-PC mutation workflow was tested through discovery and cancellation,
not through copying the largest file on the actual PC.

The [verification manifest](../EVIDENCE/routine-backup/verification-2026-10-17.json)
records the matching tested/build executable hashes and check counts. The
[optimized release build](../EVIDENCE/routine-backup/release-build-2026-10-17.txt)
passed. These two new walkthroughs do not fulfill the separate five-run original
offline acceptance requirement. No new model-inference accuracy or Laya
activation claim is made.

## Development installer

The shared output path has since been replaced by the
[model-planned workflow increment](MODEL-PLANNED-WORKFLOWS-2026-10-17.md).
Use that report for the current artifact; hashes below describe this earlier build.

The [installer build](../EVIDENCE/routine-backup/installer-build-2026-10-17.txt)
completed. Output: `target/release/bundle/nsis/Custodian_0.1.0_x64-setup.exe`.
This replaces the earlier artifact at that path. It is an unsigned development
package containing the existing pinned offline model, Node and agent runtime.
WebView2 must already be installed; the download bootstrapper remains disabled.

- Size: 1,209,930,963 bytes.
- Installer SHA-256:
  `7EB1F50784CF79FEEE2D514FFC871168705C6951301AD1ECC86DF8827F4A0550`.
- Native/tested executable SHA-256:
  `E567F5709F5FAF9D2B4CD2AC6D343A04223DBC30E1C0F62000AC64E70F885702`.

The [artifact manifest](../EVIDENCE/routine-backup/artifacts-2026-10-17.json)
records the sizes and hashes. Bundling skipped binary patching, and the native
executable still matched the outside-checkout tested copy after packaging.
The staged and outside-checkout agent files also matched; no agent/model source
was changed for this increment.

Clean-profile installation, upgrade, uninstallation, missing-WebView2 behavior
and abrupt process death during copying have not been accepted by these runs.
General desktop automation remains open. A generated installer and these two
walkthroughs do not establish whole-product completion or a hackathon win.
