# Documentation

Start with the [project README](../README.md), then use the current status and
verification records below. Implementation notes describe the increment they
tested; they do not establish that every later feature is accepted.

## Current scope and verification

- [Capability register](reference/CAPABILITY-REGISTER.md): supported routes and remaining gates.
- [Honesty register](../HONESTY.md): limitations and unavailable capabilities.
- [Hackathon delivery](verification/HACKATHON-DELIVERY-RECORD.md): the supported demonstration subset.
- [Latest audit](verification/VERIFICATION-AUDIT-2026-10-19.md): build and execution findings.
- [Repair checkpoint](verification/BUILD-REPAIR-EXECUTION-2026-10-19.md): subsequent repairs and open checks.
- [Offline VM acceptance](verification/OFFLINE-VM-ACCEPTANCE-2026-10-19.md): installation test procedure.

## Design and implementation

- [Architecture](design/CUSTODIAN_ARCHITECTURE.md) and [build plan](design/CUSTODIAN_BUILD_PLAN.md).
- [Full offline execution plan](plans/OFFLINE-EXECUTION-PLAN-2026-10-17.md).
- [Stage 0](verification/STAGE-0-RUNTIME-REPAIR-2026-10-17.md), [Stage 1](verification/STAGE-1-AUTHORITY-AND-RECOVERY-2026-10-18.md), [Stage 2](verification/STAGE-2-TASK-COORDINATOR-2026-10-18.md), and [Stage 3](verification/STAGE-3-LANGUAGE-AND-PLANNING-2026-10-18.md).
- [Stage 4](verification/STAGE-4-REAL-APP-IMPLEMENTATION-2026-10-18.md), [Stage 5](verification/STAGE-5-EXECUTION-RECORD-2026-10-18.md), and [Stage 6](verification/STAGE-6-EXECUTION-RECORD-2026-10-18.md).
- [Architecture decisions](design/decisions/) and [dependency versions](reference/VERSIONS.md).

## Maintaining the repository

- [Repository structure and source boundaries](reference/REPOSITORY-STRUCTURE.md).
- [Cleanup verification](verification/REPOSITORY-CLEANUP-2026-10-19.md).
- [Contribution workflow](../CONTRIBUTING.md).
- [Evidence retention policy](EVIDENCE/README.md).
- [Project video source](../video/README.md).

## What belongs here

| Folder          | Purpose                                                         |
| --------------- | --------------------------------------------------------------- |
| `design/`       | Architecture and design decisions                               |
| `plans/`        | Proposed work; these documents do not establish completion      |
| `reference/`    | Acceptance contract, capability register and repository policy  |
| `verification/` | Dated implementation records, results and remaining gates       |
| `EVIDENCE/`     | Frozen evaluation inputs and evidence cited by retained reports |

The dated records preserve what was tested and what failed; their dates matter.
Unreferenced run logs and duplicate HTML exports have been removed from the
versioned source tree. Local originals and their SHA-256 manifest are preserved
under `.local/repository-archive-2026-10-19/`.

New raw runs belong in ignored `artifacts/` or `target/` directories. Keep the
docs root limited to this index, and promote only reviewed, cited evidence.
