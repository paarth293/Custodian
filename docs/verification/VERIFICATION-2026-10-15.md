# Build verification — 15 October 2026

The current checkout builds and its routine automated checks pass. This does
not establish that all roadmap features or interactive acceptance criteria are
complete. Live model testing found a sandboxed chat timeout, and the model's
accuracy gate remains below target after improving from 49/57 to 52/57.

## Fixes made during verification

- Fixed missing evaluation summary variables that caused a runtime error after
  all model cases had finished.
- Fixed Rust, JavaScript and Python lint/format failures.
- Fixed inconsistent task-coordinator lock ordering, overlapping parent/child
  resource conflicts and path normalization for new destinations. Cancellation
  retains an executing worker's locks until it stops; an old completion cannot
  unlock a newer task's resource.
- Fixed file-move rollback when verification returns an I/O error; open the
  source before creating a cross-volume destination so an open failure leaves
  no empty destination behind.
- Reject quarantine manifest paths containing parent traversal, including
  paths that lexically begin inside the vault.
- Serialize agent history resets with chat requests. Report reset errors
  instead of claiming a fresh conversation succeeded; avoid loading an unused
  model merely to reset it.
- Corrected model routing guidance for live information, dates, to-dos and broad
  check-ups. Added model startup stage timings and raised the minimum context
  from 3,072 to 4,096 tokens after measuring a 3,319-token system/tool prompt.
- End duplicate-scan tracking when scanning completes. The clean-up approval
  remains a separate pending plan.
- Abstain on malformed structured ML recommendations rather than raising a
  type error. Tightened the ML stdio smoke test to require quarantine on known
  antivirus hits.

## Completed checks

| Check | Result |
| --- | --- |
| Root production JavaScript build | Passed |
| TypeScript typecheck, ESLint, Prettier | Passed |
| JavaScript tests | 92 passed |
| Rust formatting and workspace Clippy, warnings denied | Passed |
| Rust workspace tests | 296 passed, 3 opt-in tests excluded |
| File-move tests with C: and D: temporary directories | 6 passed |
| Python Ruff lint/format, mypy | Passed |
| Python tests, including ML stdio integration | 21 passed |
| Secret scan | 0 findings |
| Rust dependency advisory/license/source policy | Passed; duplicate-version warnings remain |
| Production JavaScript dependency audit | 0 vulnerabilities |
| Full JavaScript dependency audit | 2 moderate development dependency issues; no published fix |
| Python dependency audit | No known vulnerabilities |
| Native production executable build | Passed |
| Windows NSIS installer build | Passed; 1,975,632 bytes |
| Native launch smoke test | Passed: Cubby window opened and responded for the 12-second observation |
| Real local model under Windows job limits | Passed |
| Sandboxed CPU model integration | Failed: no response within the 120-second inactivity watchdog |
| Full 57-case Qwen3 4B evaluation, revised routing prompt | 52/57 (91%); fails the 95% gate |

The model evaluation uses simulated desktop results. Its 57/57 safety checks
cover the harness's deletion-claim checks and are not a general security proof.
Do not lower the acceptance threshold or rewrite expected answers to hide the
five remaining failures. See `target/verification-model-eval-final.log` for
individual cases. The baseline run was 49/57 (86%); a single measured
improvement is not a guarantee that repeated runs will produce the same score.

## Remaining acceptance work

- Resolve sandboxed model startup/inference latency: the smaller CPU model
  reaches tool-prompt preparation, then exceeds the unchanged 120-second
  inactivity deadline. An isolated CPU retry also failed. A CPU attention
  adjustment did not resolve this and was reverted. Explicit Vulkan startup
  stalled inside native backend initialization and was stopped; unrestricted
  fallback was not enabled.
- Raise model tool/argument/answer accuracy to the acceptance target.
- Exercise the running desktop's task panel, approval clicks, stop behavior and
  fresh conversation control interactively.
- Verify installer installation and uninstallation on a clean Windows profile.
- Implement remaining features documented in `HONESTY.md`, including durable
  task recovery and persisted undo for new file actions. Online browser and
  installation workflows still have placeholders.

The full dependency audit reports `stream-json` advisories
GHSA-hqr4-qq8f-hg3x and GHSA-mjw6-4jj6-33hc, pulled in by development-only
`quicktype`. The runtime-only audit is clean. No force-upgrade or advisory
suppression was applied.

## Environment and evidence

Windows checkout at `D:\Custodian`; Node 24.18.0, npm 11.16.0, Rust 1.97.0,
Python 3.11.9. Python checks have not been repeated on the repository's 3.12
target because that interpreter is not installed locally.

Detailed logs are local, ignored build outputs:

- `target/verification-rust-tests.log`
- `target/verification-cargo-deny.log`
- `target/verification-real-model.log`
- `target/verification-model-eval.log`
- `target/verification-model-eval-final.log`
- `target/verification-installer.log`
- `target/verification-sandbox-diagnostic.log`
- `target/verification-sandbox-cpu.log`
- `target/verification-prompt-size.log`
- `target/verification-startup.json`
- `target/verification-sandbox-vulkan.log`

Installer: `target/release/bundle/nsis/Custodian_0.1.0_x64-setup.exe`.
The local Node agent and model files are still not bundled; a successful
installer build does not establish standalone installed AI chat support.

The launch smoke test verifies process/window startup only; it does not verify
interactive chat, approval clicks, installation or all user workflows.
