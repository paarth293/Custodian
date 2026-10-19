# Verification — 16 October 2026

Historical snapshot: the installer at the output path below has since been
replaced. Current capability restrictions, package hashes and checks are in the
[offline foundation report](OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md).
The earlier model scores below are not a certification of that newer package.

This report supersedes the 15 October results only for the checks below.
Custodian is **not accepted as the complete general-purpose assistant**.
The completion requirements remain in [ACCEPTANCE.md](../reference/ACCEPTANCE.md).

## Whole-PC scope

Read-only file discovery defaults to all accessible local drives, including
removable drives. An explicit drive or absolute folder path is respected,
including paths outside the six personal-folder shortcuts. Whole-system wording
takes precedence over an incidental personal-folder name. Model-generated
folders cannot silently narrow the scope requested by the person.

Whole-system discovery includes ordinary hidden and system directories when
Windows permits reading them. It does not elevate privileges, follow reparse
points, hydrate cloud placeholders, or access network shares. Unreadable and
excluded entries are reported; a scan is not proof that every file was readable.
Whole-system discovery has no arbitrary file-count cutoff and remains cancellable.
Cleanup retains its more conservative scope and exclusions.

File changes use a separate operation policy. Ordinary local paths and
cross-drive tasks are supported. Protected operating-system, application,
credential and boot locations are refused. Approval captures exact paths and
source identity/content; execution checks them again, refuses overwrites, saves
a sealed prepared receipt, verifies the result and records undo. These controls
reduce risk; unresolved path races and other audit findings preclude a guarantee
that every possible operation is harmless.

## Actual desktop evidence

`scripts/verify-desktop.ps1` drives the Windows accessibility interface and real
chat/approval buttons. It creates new marked fixtures and never uses personal
files as mutation fixtures.

The cross-drive run passed:

1. Find a measured 8,192-byte file in a custom folder on C:.
2. Ask to move “it” to an arbitrary fixture folder on D:.
3. Decline the approval; verify both locations and original hash are unchanged.
4. Ask again and approve; verify the source is gone and destination hash matches.
5. Restart the desktop with the same isolated profile, request undo and verify
   identical original bytes are restored.

Evidence: `target/acceptance-live-desktop-2026-10-16.log`. The profile was
`C:\Users\paagu\.codex\visualizations\2026\10\16\01a142c7-c123-7122-873a-8f2707fc4063\desktop-acceptance-02`;
the destination was `D:\Custodian\fixture\desktop-acceptance-other-drive-02`.

The first live attempt exposed missing history-directory creation in a fresh
profile; that defect was fixed and regression-tested. The harness now isolates
WebView data and waits for its own browser children to exit before reopening.
This restart check does not simulate process death during a file mutation.

The current-source repeat also passed a real whole-system read: the desktop
task panel showed C: and D:, an independent storage question was answered while
the scan ran, and Stop cancelled it within the ten-second assertion window.
The original fixture remained byte-identical. Evidence:
`target/acceptance-live-whole-system-2026-10-16.log`; isolated profile
`desktop-acceptance-06` under the same artifact directory. Earlier repeated
harness attempts lost simulated keystrokes; accessibility value entry now sets
and verifies the entire input before clicking the enabled Send button.

The final optimized release executable repeated all five assertions successfully
using `desktop-acceptance-07` and a new D: destination fixture. Evidence:
`target/acceptance-live-release-2026-10-16.log`. Installer metadata patching
changes the executable hash; final artifact hashes are recorded below.

## Real model evaluations

The original 57 requests and expected answers were retained. The gate requires
at least 95% without rounding and a complete run. Desktop tool results in this
harness are simulated; these scores are not proof of actual file execution.

| Model | Result | Designated deletion-claim checks |
| --- | --- | --- |
| Bundled Qwen2.5 1.5B Q4_K_M, CPU | 56/57, 98.25%, gate passed | 4/4 |
| Optional Qwen3 4B Q4_K_M, Vulkan | 56/57, 98.25%, gate passed | 4/4 |

The JSON reports retain each miss, model hash, load time and per-case timings:
`target/acceptance-model-cpu-final-2026-10-16.json` and
`target/acceptance-model-4b-final-complete-2026-10-16.json`.
There are four designated deletion-claim checks, not 57 independent safety
checks. Neither score is a general security evaluation or a latency guarantee.
The CPU miss used lowercase `documents` where the unchanged case expected
`Documents`; the desktop accepts either spelling. The 4B miss omitted the Videos
scope for “which videos are the chunkiest?”. The desktop now binds read scope
from the person's original request rather than accepting model-selected roots;
that binding has separate regression coverage. Both misses remain in the reports.

## Packaging and limits

The staged offline runtime includes Node 24.18, the compiled agent, 133 runtime
packages and the SHA-256-pinned CPU model. A real AppContainer CPU knowledge/tool
smoke passed under the unchanged 120-second inactivity deadline. An isolated
runtime outside the source checkout also answered inside AppContainer.

A native release and runtime-inclusive NSIS installer were produced during this
session. The final native release built successfully from the current source in
4 minutes 53 seconds. Evidence:
`target/acceptance-native-release-final-2026-10-16.log`.
The final NSIS bundle succeeded with Zlib compression after the interrupted
LZMA packaging attempt. Evidence:
`target/acceptance-installer-zlib-final-2026-10-16.log`.

- Installer: `target/release/bundle/nsis/Custodian_0.1.0_x64-setup.exe`.
- Size: 1,209,995,884 bytes (1.13 GiB), unsigned development artifact.
- Installer SHA-256:
  `d9f334bff8235fb44e29eda430a49599be1eec549e70feb8ac051d12a1699076`.
- Native executable SHA-256 after packaging:
  `fe80d94f2977c889879115663281293f6e54383ba0045b545ae625a6c0875f01`.

Machine-readable evidence: `target/acceptance-artifacts-final-2026-10-16.json`.
The executable with that exact final hash passed all five live assertions again
after packaging, using `desktop-acceptance-08`. Evidence:
`target/acceptance-live-packaged-release-2026-10-16.log`.
Generating an installer does not establish clean-profile installation.
Windows Sandbox is unavailable on this machine, and clean-profile install,
offline operation and uninstall remain unverified.

The final isolated staged-runtime check also exercised a real tool callback and
passed in 106.84 seconds including model verification/loading. It used Node,
agent, dependencies and model outside the checkout, with GPU disabled and the
default AppContainer; no unrestricted fallback was enabled. Evidence:
`target/acceptance-isolated-runtime-final-2026-10-16.log`.

## Final routine checks

- Rust workspace: 315 passed, zero failed, four environment-dependent tests
  ignored in the routine run. The isolated-runtime test above was run explicitly.
- JavaScript: 121 passed (desktop 24, agent 90, broker 7).
- Build, types, lint and Rust Clippy with warnings denied passed.
- Python was not rerun for this change; the older report retains its own result.

Evidence: `target/acceptance-workspace-rust-final-2026-10-16.log`,
`target/acceptance-clippy-final-2026-10-16.log` and
`target/acceptance-js-final-2026-10-16.log`.

Full dependency audit reports zero known vulnerabilities, and schema generation
still succeeds after replacing the quicktype CLI dependency with quicktype-core.
Evidence: `target/acceptance-audit-2026-10-16.json` and
`target/acceptance-proto-2026-10-16.log`.
The generation probe writes to `target/proto-generator-acceptance`; checked-in
wire-type adaptations are retained. Raw generated output still needs the
repository's formatting and type adaptations before replacing those sources.

Remaining acceptance includes abrupt mid-operation crash recovery, repeated
walkthroughs, clean-profile installation, broader settings coverage, real online
workflows, semantic search/parsing, voice and the other documented placeholders.
See [HONESTY.md](../../HONESTY.md) and the
[open audit findings](FIX-INVENTORY-2026-10-15.md). Whole-PC scope does not mean
arbitrary shell execution or completion of unsupported capabilities.
