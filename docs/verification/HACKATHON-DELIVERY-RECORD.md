# Custodian hackathon delivery record

This record describes the strongest demo-ready subset of the current checkout.
It is intentionally not a claim that Stages 4, 5 or 6 are complete.

## Demo promise

Custodian is an offline, safety-first local assistant. It can measure a
permitted selection, show the exact consequence, obtain native approval,
perform a bounded file effect, verify the result, retain a receipt and refuse
stale, private, conflicting or unsupported work. Model text and document text
do not grant authority.

## Reproducible fixture

Create an isolated Cedar fixture with:

```powershell
python scripts/create-cedar-hackathon-fixture.py "$env:TEMP\custodian-cedar-demo"
```

The fixture contains five public files, one private canary and one untrusted
instruction-like file. The script writes only beneath the requested output
root and emits `manifest.json` with SHA-256 hashes and expected assertions.

## Five-minute walkthrough

1. Show the public Cedar files and the private canary without opening the
   canary through Custodian.
2. Ask Custodian to find the Cedar files while excluding `private`.
3. Show the measured approval preview: exact paths, count, bytes, exclusion,
   destination and recovery behavior.
4. Approve a copy or move that preserves originals; show verified completion
   and receipt-backed undo.
5. Create a destination conflict or change an approved source and execute.
   Show refusal/partial reporting rather than substitution or overwrite.
6. Change a verified output after completion and request undo. Show that the
   later edit is retained and the receipt remains available for review.
7. Optionally show one already-qualified narrow VS Code or Excel workflow only
   after verifying it on the exact demo machine. Never demo Word/DOCX.

## Demo-safe capabilities

- Local permitted-file discovery and metadata selection.
- Private-path and protected-path refusal.
- Exact approved copy/move/rename/folder operations.
- Hash/identity checks, receipts, progress and recovery inspection.
- Safe undo that refuses to destroy later user edits.
- Catalogued reversible user settings such as file-extension visibility or
  theme, subject to Windows availability and approval.
- Existing bounded largest-file backup routine with explicit local scope and
  permission; it is not a general scheduler or ransomware-proof backup.
- Narrow qualified text-editor and Excel workflows when the required Windows
  applications and fixture environment are present.

## Do not claim or demo

- Word/DOCX creation or editing.
- General PDF/PPTX generation.
- OCR, image transformation, audio/video conversion or transcription.
- General desktop/UI automation or screenshots.
- Qualified Windows Recycle Bin deletion/restoration.
- General backup/restore or ZIP workflows through the normal chat route.
- Universal application support, unrestricted shell access, online actions or
  credential handling.
- Completed Stage 4, Stage 5 or Stage 6 acceptance.

## Verification state

Passing focused checks include the core Rust tests, workspace Clippy, JavaScript
build/typecheck/tests, Python tests and secret scan as recorded in
`docs/verification/WHOLE-BUILD-VERIFICATION-2026-10-18.md`. The native desktop workspace
build was attempted but the host terminated Rust compilation during the
resource-heavy desktop build; it is not called passed. Docker is unavailable
in the current environment.

The current delivery-focused rerun also passed:

```text
python -m pytest tests/test_cedar_hackathon_fixture.py  # 2 passed
cargo test -p custodian-core --locked                       # 207 passed, 2 ignored
cargo clippy -p custodian-core --all-targets --locked -- -D warnings
npm run build
npm run typecheck
npm test -- --run
npm run lint
python tests/security/scan_secrets.py                       # 0 findings
git diff --check
```

The detailed logs are retained under `target/hackathon-*.txt`. The existing
task board exposes evidence details and Stop/Pause/recovery behavior; its
current working-tree changes were preserved.

## Prerequisites and stage status

- Stage 4: narrow real-app workflows exist; optimized repeated acceptance,
  installer and disposable offline installation remain open.
- Stage 5: bounded storage primitives and tests exist; product routes,
  Windows recycle provider and packaged disruption evidence remain open.
- Stage 6: bounded TXT/Markdown/CSV content core exists; the content route,
  contained parser workers, office/media formats and package assets remain open.

The correct judging claim is a reliable safety-oriented prototype with real
verified subsets, not an unrestricted autonomous computer operator.
