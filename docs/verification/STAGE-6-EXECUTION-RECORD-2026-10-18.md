# Stage 6 execution record

Date: 18 October 2026. This is the durable continuation record for Stage 6.
It distinguishes independently verified core work from product-route and
packaging acceptance. Code presence and unit tests do not by themselves accept
the stage.

## Prerequisites

- Stage 4: **not accepted**. The current real-application report qualifies a
  narrow VS Code text workflow and strict Excel subset, but the stage still
  lacks the required repeated optimized/package and installation evidence.
- Stage 5: **incomplete**. Batch archive, versioned backup/restore and governed
  recycle are exported core primitives, but desktop routes are not connected;
  the Windows recycle provider is unavailable. Stage 6 does not upgrade either
  prerequisite.

## Capability map

| Area | Current implementation | Status | Exact remaining gate |
|---|---|---|---|
| 6.0 contracts/provenance | `crates/custodian-core/src/content.rs` typed `ContentRequest`, provenance, source version/hash, locations, warnings and partial status | core implemented/tested | contained parser worker, model exposure route, packaged worker limits |
| 6.1 TXT/Markdown/CSV | bounded local reader, UTF-8/UTF-16, newline handling, multiline CSV, typed values, malformed-row warnings, staged writer and structured reopen verifier | verified core subset | native planner/desktop route and end-to-end showcase |
| 6.2 DOCX/PDF/XLSX | existing strict XLSX/workbook and Excel subsets remain available; no DOCX/PDF reader | partial | DOCX/PDF dependencies/workers and full XLSX content route |
| 6.3 artifact writers | TXT/Markdown/CSV create-new staged writer only | partial | format-aware DOCX/PDF/XLSX/PPTX writers and qualified edit preservation |
| 6.4 independent verification | structured reopen verification for text/CSV; existing workbook and app verifiers | partial | independent office/media renderers and false-success suite |
| 6.5 planning/showcases | existing Stage 3 planner and Stage 4 app workflows; content module not connected to language/planner | open | typed content capability registry, receipts, handoff/report/dataset routes |
| 6.6 bounded repair | existing coordinator/app recovery foundations; no content replan route | open | two-cycle budget, stale-source reconciliation and packaged stop/restart evidence |
| 6.7 media | no image/OCR/audio/video implementation or pinned assets | unavailable | bundled codecs/OCR/image libraries/models and acceptance fixtures |
| 6.8 showcase families | Stage 5 archive core and Stage 4 app/XLSX subsets exist separately | not demonstrated | same registry through all three real product routes |

## Implemented core evidence

The text/Markdown/CSV module was added and exported in commit `f78d5c8`.
It deliberately does not allow content to choose destinations, grant authority,
or issue instructions. It rejects non-local/non-regular/private sources, caps
input at 64 MiB, caps decoded output at 128 MiB, records source identity and
BLAKE3 hash, and preserves formula-like CSV values as text.

Focused cases cover Unicode, UTF-16, mixed newlines, multiline quoted CSV,
malformed rows, formula-like values, changed sources, private refusal and
staged structured verification. The module's tests run inside the core suite.

## Checks

- `cargo test -p custodian-core --locked` — passed in the implementing session:
  207 passed, 2 ignored.
- `cargo clippy -p custodian-core --all-targets --locked -- -D warnings` — passed.
- `cargo fmt --all -- --check` — implementing session reported pass before
  unrelated worktree formatting was reintroduced; full current worktree status
  must be rerun before delivery.
- `cargo test --workspace --locked` — attempted after the content change, but
  the host terminated the concurrent/serial workspace rustc process while
  compiling the desktop crate. The retained logs are
  `target/stage6-workspace-tests.txt` and
  `target/stage6-workspace-tests-serial.txt`; this is an environment-limited
  check, not a passing workspace result.
- Stage 4/5 prior workspace evidence remains historical and is not repeated as
  Stage 6 acceptance.

## External blockers and unsupported claims

1. No pinned PDF, DOCX, PPTX, image, OCR or media dependency/worker bundle is
   present. These capabilities remain unavailable rather than being simulated.
2. The parser binary and agent `readerExtract` route are not a connected typed
   content pipeline; they must not be relabelled as Stage 6 extraction.
3. No contained read-only parser process with memory, time, decoded-data and
   cancellation enforcement is integrated with the desktop route.
4. Python requirements are lower-bound-only and the package staging script does
   not stage Stage 6 parser/media assets.
5. Stage 4 and Stage 5 acceptance remain open, so Stage 6 cannot be accepted.

## Next executable action

Connect `content` through a typed native capability registry and task
coordinator route using task-owned staged outputs and receipts, while keeping
DOCX/PDF/PPTX/media unavailable. Then qualify the existing strict XLSX route
as content provenance rather than claiming general spreadsheet support. A
separate dependency and licensing decision is required before adding external
format/media workers.

Stage 6 is **incomplete**. The verified subset is the exported offline
TXT/Markdown/CSV core plus previously qualified narrow XLSX/editor components.
