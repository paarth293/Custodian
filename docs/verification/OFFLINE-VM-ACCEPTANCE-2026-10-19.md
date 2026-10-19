# Disposable Windows acceptance environment

Selected environment: a full disposable Windows VM with a recoverable snapshot,
ordinary-user desktop access, and a disconnected virtual network adapter.
No VM has yet been discovered or provisioned. This document is a test
specification, not evidence that installation ran.

The current Home Single Language host has no discovered hypervisor or guest.
Windows Sandbox is unsupported on Home according to
[Microsoft's requirements](https://learn.microsoft.com/windows/security/threat-protection/windows-sandbox/windows-sandbox-overview).
Sandbox would also require separate qualification of Office setup and guest
restart behavior. Use a full VM for the complete gate. Do not install or test
uninstall against the normal development host as a substitute.

## Guest preparation

Use properly licensed Windows and Excel media. Provision a current supported
x64 Windows guest, sufficient memory for the bundled CPU model and application
workload, and at least two ordinary synthetic working directories outside the
checkout. Record the actual OS, resources, application versions and snapshots.
Use VS Code 1.140.0 and an exactly qualified Excel patch. The new
16.0.20430.20182 patch currently has native workflow evidence, with optimized
desktop acceptance still pending; do not describe it as fully qualified yet.

Prepare an offline prerequisite-present snapshot with WebView2, Code and Excel
installed and activated through their normal supported mechanisms. Keep a
separate prerequisite-missing snapshot for the installer refusal check. Do not
remove system WebView2 from the host or reuse a guest with valuable files.
Include unrelated synthetic documents and one unsaved synthetic application
document as preservation canaries. Disconnect the guest's virtual NIC before
acceptance. Record the hypervisor's disconnected configuration and guest
adapter/route state; a failed web request alone does not prove isolation.

Transfer the final candidate, its runtime, frozen expectations, fixture scripts,
sanitized workbook, and independently recorded hashes by read-only media or a
read-only share. Do not map the host's Documents, home directory or checkout
writable. Keep guest output inside the guest; export only the reviewed evidence.
Do not use the historical audited candidate as the repaired release.

## Run and preserve evidence

1. Before installation, inventory the package and runtime hashes and preserve
   canary hashes. Record the installer identity, guest OS and prerequisite
   versions. Keep every attempt in a fresh evidence directory.
2. In the prerequisite-missing snapshot, confirm that offline setup exits with
   its documented WebView2 prerequisite failure and does not fetch components.
   A portable executable check cannot substitute for installer execution.
3. Restore the prerequisite-present snapshot. Install offline, recording actual
   exit code and destination. Launch as an ordinary user with no checkout,
   development PATH, model download or development dependency available.
4. Run the existing frozen desktop baseline and real application repetition
   gates from guest-local copies of `scripts/verify-desktop.ps1`. Supply the
   actual installed executable, fresh fixture/destination roots, the sanitized
   workbook, and `-ApplicationRuns 5` for `-AppTasksOnly`. Both applications
   must pass. Retain approval, privacy, Stop, revocation and recovery failures;
   do not change deadlines or assertions to accept the installed build.
5. Restart the guest, relaunch as the ordinary user and verify persisted task
   evidence, appropriate recovery and safe undo. Exercise interruption at a
   defined receipt boundary using synthetic files. Require preserved original
   and unrelated data and no unsupported automatic effect replay.
6. Uninstall and verify unrelated data/canaries and documented retention or
   removal of Custodian-owned state. Separately test upgrade preservation if
   the release claims compatible upgrade/recovery behavior.
7. Export logs, all failed attempts, hashes, actual process results, guest
   configuration, and before/after observations. Review secret/user metadata
   before placing evidence in the repository. Revert the disposable guest.

Artifact verification (`scripts/verify-stage-four-delivery.py`) remains a
separate check. Its `installationAccepted: false` must not be changed merely
because artifact hashes pass. No installed-product acceptance is claimed until
the actual guest steps above have executed against the final candidate.
