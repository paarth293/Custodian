# Offline implementation sequence

Scope decision: 16 October 2026. Online work is deferred by the project owner.
This is an implementation sequence, not a declaration that these steps work.
It supplements the existing detailed offline roadmap and capability audit.

The expanded [offline hackathon master plan](OFFLINE-HACKATHON-MASTER-PLAN.md)
now defines the delivery milestones, architecture decisions, evaluation, demo
and release approach. Its [acceptance matrix](OFFLINE-HACKATHON-ACCEPTANCE-MATRIX.md)
provides the proposed pass/fail cases. Both are planning documents, not executed
implementation or new verification evidence.

The owner subsequently authorized implementation. The first policy/offline
foundation is implemented with bounded verification; see the
[implementation report](../verification/OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md).
The remaining milestones and full product acceptance are still open.

Implementation has since begun: see the
[first foundation package and remaining gates](../verification/OFFLINE-FOUNDATION-IMPLEMENTATION-2026-10-16.md).

## Step 1 — Offline release contract

Status: scope recorded; enforcement remains to be implemented and verified.

- Initial release target: Windows, text interaction, local open-weight model.
- Operate on accessible local drives and arbitrary local working folders,
  subject to OS permissions, private zones and operation-specific protections.
  Personal-folder shortcuts are conveniences, not the product boundary.
- Build reusable file and local-app actions that compose into unfamiliar tasks.
  Example workflows are acceptance samples, not the only supported commands.
- Custodian must not initiate browser searches, URL handoffs, uploads, account
  actions, cloud-model calls or other external transfers in offline mode.
  Audit native handoffs as well as the already sandboxed model process.
- Third-party applications can have independent network behavior. Offline
  acceptance must run with connectivity unavailable and must not claim that
  model sandboxing isolates every application on the computer.
- Keep online integration code for future development; exclude its unfinished
  functionality from offline completion claims and block offline entry points.
- Treat source material as data, never as permission or trusted instructions.
- No always-on administrator privileges; harmful/protected changes are blocked.
- Defer voice and additional OS platforms until the Windows text workflow works.

## Ordered implementation gates

| Step | Work | Completion evidence | Current status |
| --- | --- | --- | --- |
| 2. Permissions and private zones | One policy boundary used by existing reads and actions; persistent user private zones; default credential exclusions; scoped/revocable authority; offline transfer denial | Synthetic private files cannot appear in search, metadata, previews, history, model context or app observations. Direct/model routes obey the same rules. Aliases/reparse tricks do not bypass denial. Revocation and forbidden network handoffs fail closed. | Next implementation step; missing |
| 3. Dependable file execution | Route existing operations through policy; consistent prepare/approve/execute/verify/receipt/undo; batch plans with per-item outcomes; repair remaining cleanup/settings write ordering | Current cross-drive journey still passes; declined/stale plans make no changes; interrupted operations reopen honestly; rollback never overwrites newer user data. | Existing subset works; expansion/hardening pending |
| 4. Local file content and artifacts | Sandboxed supported-format readers; create/edit text and structured local outputs; archive support; validated paths and bounded input/output | Actual artifacts reopen and match requested contents; originals survive failed edits; document instructions cannot authorize additional actions; unsupported/encrypted/unreadable formats are reported honestly. | Mostly missing |
| 5. Local application control | Prefer native/app APIs, then Windows accessibility automation; scoped window/element discovery and actions; approved application access; observe results | Execute a real workflow in at least one supported local app on changed inputs; verify resulting app state or artifact. Never blindly repeat an uncertain click or touch excluded apps/screens. Work without internet. | App launch/list/close subset exists; general control missing |
| 6. Multi-step offline task loop | Typed plans composed from reusable capabilities; observations and preconditions between steps; stop/pause at safe boundaries; bounded retries and honest recovery | Complete varied, previously unseen local tasks spanning files and an app; stop prevents later steps; failures show partial outcomes. Each claimed result has execution evidence. | General operator loop missing |
| 7. Useful offline routines and supported settings | Add validated settings/app adapters according to tested user needs; persistent schedules/preferences; clear per-task authority | Each exposed capability executes, verifies and recovers in the live app. Scheduled work respects current permissions and private zones. | Six settings and reminders exist; broader work pending |
| 8. Offline acceptance and packaging | Test final installer/profile, real model/tool integration, accessibility and responsiveness; representative workflows and adversarial fixtures | Clean-profile offline install/use/uninstall; five repeated core journeys; held-out workflow results and failures recorded; privacy cases pass; Stop/restart/undo verified on final artifacts. | Existing release evidence retained; full offline acceptance open |

Do not mark a step complete because code compiles, a model selects a tool, a
helper returns “executed,” or one scripted example works. A failed gate keeps
the relevant capability incomplete. Existing implementation may be reused,
but does not substitute for the gate's evidence.

## First implementation slice: Step 2

1. Define a typed operation request and policy decision independent of model
   text: action, target/object identity, scope, task authority, risk and outcome
   of allow/deny/require approval. Denial must happen before reading the target.
2. Store private-zone choices and default credential restrictions. Resolve
   equivalent paths consistently and refuse uncertain/path-race cases.
3. Apply policy to existing filename/largest discovery, file inspection, file
   preparation/mutation, app access and context/history generation. Do not log
   denied paths or contents as a way of reporting their exclusion.
4. Add a simple native-app preference interface to manage these choices and
   revoke task authority. Policy enforcement must not depend on that UI being open.
5. Reject existing website/search/network handoffs in offline mode before an
   external application or transfer starts. Model constraints alone are insufficient.
6. Test policy with newly created synthetic public/private fixtures, including
   direct requests, model calls, stale approvals and alias/reparse attempts.
7. Repeat the known working file journey outside private zones. Broad local
   access must still work; security is not implemented by reducing everything
   to Documents and Downloads.

Step 2 is the immediate priority. Document reading, wider app automation and
script execution must use this boundary before being exposed to the model.

Broader offline software/development work belongs after the basic execution
loop: add a restricted local command/script runner with explicit executable,
argument, filesystem and resource authority, then validated Git/build and
application-management adapters. Arbitrary shell text is not a substitute for
this work. Install/update operations requiring downloads stay deferred with
online scope; local installers need their own verification and approval path.

## Progress evidence

Keep a small capability matrix with four explicit states: not implemented,
implemented but unverified, verified for a stated scope, and deferred. Record
the supported inputs, paths/apps, evidence, failure behavior and recovery for
each capability. Offline release acceptance is separate from completion of
the long-term online-plus-offline vision.

References: [current capability audit](../verification/VISION-CAPABILITY-AUDIT-2026-10-16.md),
[execution evidence](../verification/VERIFICATION-2026-10-16.md),
[detailed roadmap](CUSTODIAN_OFFLINE_ASSISTANT_IMPLEMENTATION_ROADMAP.md),
[existing acceptance requirements](../reference/ACCEPTANCE.md).
