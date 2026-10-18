# Whole-build verification: 18 October 2026

This is a verification report, not a stage acceptance report. Existing user
changes and historical evidence were preserved. Commands were run from
`D:\Custodian`.

## Results

| Command | Result | Evidence |
|---|---|---|
| `npm run build` | passed | `target/verify-npm-build.txt` |
| `npm run typecheck` | passed | `target/verify-npm-typecheck.txt` |
| `npm test -- --run` | passed: desktop 27, agent 104, broker 7, editor adapter 6 | `target/verify-npm-tests.txt` |
| `cargo fmt --all -- --check` | passed in this run | `target/verify-cargo-fmt.txt` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | passed; environment hard-link warnings only | `target/verify-cargo-clippy.txt` |
| `cargo test -p custodian-core --locked` | passed: 207, 2 ignored | prior output and Stage 6 record |
| `cargo build --workspace --locked -j 1` | not completed; host terminated rustc while compiling desktop | `target/verify-cargo-build.txt` |
| `cargo build -p custodian-desktop --locked --features tauri/custom-protocol` | not completed; host terminated rustc while compiling desktop | `target/verify-desktop-build.txt` |
| `cargo test --workspace --locked` | not completed; host terminated rustc during desktop compilation | `target/stage6-workspace-tests.txt`, `target/stage6-workspace-tests-serial.txt` |
| `python -m pytest` | passed: 70, Python 3.11.9 | `target/verify-python-tests.txt` |
| `python tests/security/scan_secrets.py` | passed: 0 findings | `target/verify-secret-scan.txt` |
| `ruff check .` | failed on pre-existing/unrelated `scripts/prepare-stage-four-workbook-fixture.py` UP012 | `target/verify-ruff-check.txt` |
| `ruff format --check .` | failed on the same pre-existing/unrelated fixture script | `target/verify-ruff-format.txt` |
| `mypy .` | passed: 30 source files | `target/verify-mypy.txt` |
| `docker --version` | unavailable: command not found | terminal result |

The JavaScript production build succeeded. Native workspace and desktop builds
were not proven because the host terminated the Rust compiler process during a
resource-heavy desktop build; no Rust diagnostic error was emitted. This is not
reported as a passing native whole build.

## Why Stage 4 is incomplete

The Stage 4 report explicitly records one development workflow per app, not the
mandatory final acceptance. Still open:

- repeated optimized desktop acceptance for both editor and Excel workflows;
- final regression through the optimized runtime;
- current installer identity and disposable offline installation;
- clean install/upgrade/uninstall evidence;
- full app/environment matrix and retained failure classification.

The implementation is also intentionally narrow: VS Code bounded UTF-8 text
editing and a strict single-sheet Excel/A1:P16 subset, not general application
control, general document editing, screenshots, arbitrary UIA, or broad Office
support.

## Why Stage 5 is incomplete

The core modules exist and their unit tests pass, but acceptance requires actual
product routes and recovery evidence. Still open:

- archive, backup/restore and recycle capabilities are not connected to the
  desktop language planner, permission cards, coordinator receipts and package;
- the Windows Recycle Bin provider is deliberately unavailable, so no real
  recycle/restore drill exists;
- cross-volume and removable-drive loss tests are not accepted;
- actual process termination at archive/backup journal/publication boundaries
  is not demonstrated;
- optimized packaged C:/D:/custom-location acceptance is not demonstrated;
- full conflict, locked/read-only, long-path and recovery matrix is open.

The Stage 5 unit tests prove core behavior only; they do not prove the intended
user-to-native-route-to-receipt flow.

## Why Stage 6 is incomplete

The verified content subset is limited to native TXT/Markdown/CSV parsing and
staged verification. Still open:

- no connected typed content reader/planner/model route;
- no contained parser worker with enforced no-network/no-shell, memory, time,
  cancellation and process-tree limits;
- no packaged parser/renderer assets;
- DOCX and PDF are unavailable;
- PPTX creation/rendering is unavailable;
- image transformation is unavailable;
- OCR and language models are unavailable;
- audio/video metadata/conversion is unavailable;
- the handoff, grounded-report and dataset showcases are not integrated through
  one registered capability graph;
- no two-cycle content replan/reconciliation route;
- no 24/25 additional end-to-end acceptance run;
- no optimized offline installation/package evidence.

## Overall conclusion

The repository has a passing JavaScript build/typecheck/test path, passing core
Rust tests and clippy, passing Python tests/security scan/mypy, and a native
format check. It does **not** have a completed whole native workspace build in
this environment because the host terminated desktop Rust compilation. More
importantly, Stages 4–6 are incomplete primarily due to unmet behavioral and
delivery acceptance gates, not merely unit-test failures. The exact status is:

- Stage 4: narrow real-app implementation, final acceptance open.
- Stage 5: tested core storage primitives, product integration and provider/
  package recovery acceptance open.
- Stage 6: tested TXT/Markdown/CSV core subset, most required formats/media,
  integration and prerequisite acceptance open.
