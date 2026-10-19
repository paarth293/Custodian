# Stage 4 supervised native worker

18 October 2026. This increment implements and verifies the worker transport
foundation. Stage 4 remains open: no real editor or spreadsheet adapter is
accepted, and no app content observation is enabled in the desktop.

The user deferred Docker verification and authorized the next Stage 4 increment.
The earlier [foundation result](../EVIDENCE/stage-4/result.json) remains historical;
the worker's final evidence is recorded separately in
[worker-result.json](../EVIDENCE/stage-4/worker-result.json).

## Implemented boundary

`custodian-appworker` is a native, one-request executable. `WorkerProgram` opens
and BLAKE3-pins the native-configured executable, keeping a read-shared image
handle to prevent direct replacement/writes while held. Goals/model responses
cannot select the executable, pin, command line, environment or operation code.
This is not release signing or protection against a malicious same-user host.

The supervisor creates a suspended process with only its three private stdio
handles inherited, assigns its Windows Job before resuming, and measures its
PID/creation identity. The Job limits memory to 512 MiB, CPU to 25 percent and
active processes to one; closing it terminates its worker. It never places a
user's editor or an unrelated process in that Job. The worker runs as a trusted
normal-user process: these are resource limits, not filesystem/network isolation.
The existing agent continues to use its separate AppContainer path.

Each invocation uses a fresh random key delivered only through the private
stdin pipe, an authenticated frame and a fresh request nonce. Requests and
replies use strict typed schemas; the host rejects oversized frames before
payload allocation, limits the payload to 256 KiB and stderr to 4 KiB, and
requires exactly one matching response and successful process exit. No worker
stderr or document contents are copied into host error messages. Exit code 259
is treated as an exit code after checking the process handle's termination
signal; readable process metadata alone never establishes that a process lives.

The host polls private pipes and checks the current consent, Stop and deadline.
A blocked stdin writer cannot block that watchdog. On failure it closes the
worker Job, terminates its exact process handle and requests writer I/O
cancellation. Writer completion and process termination each receive a bounded
two-second cleanup wait. There is no unbounded join or automatic effect replay;
unproved cleanup poisons the worker client so it cannot dispatch again. Startup
error paths also terminate/check their suspended process. Deadlines are checked
before launch and resume, as well as while running. Synchronous OS process
creation and the native checkpoint callback are not preemptible; measured test
bounds do not promise a universal wall-clock guarantee during an OS/storage hang.

`ApplicationWorker` implements the existing checked adapter interface. Admission,
fresh remeasurement, exact target/document/version binding and receipt checking
stay in that gate. Execute dispatch requires the native owner's
`PendingDispatch::record_pending` callback first, binding the nonce and request
digest. The callback is an integration contract, not a finished durable app
journal. Tests implement it using create-new, write and sync on a synthetic
checkpoint. A crash after an effect preserves that checkpoint; no replay occurs.
Returned effect receipts remain available to the outer recovery gate if consent
changes after an operation returns.

The production worker supports only its own process metadata and creating,
checking and destroying its own empty message-only window. The latter is not a
user application, visible desktop UI, UI Automation or content observation.
Remeasure/Execute for actual app requests always return unavailable. They never
echo claimed identities as if measured. No arbitrary shell, global input,
screenshots, model calls or application launcher is exposed.

## Verification and retained failures

The integration suite checks native worker authentication/exit, the owned-window
probe, bad executable pins, preexisting Stop, invalid timeout and refusal of an
unqualified app before the effect checkpoint. A separate opt-in fault executable
is built only as a test example. It is not linked into or installed with the
production worker. Fault controls come from its disposable fixture directory,
never production environment variables or command-line switches.

Its 16 scenarios cover a hang, partial frame, oversized frame, wrong nonce,
wrong version, extra output, bad authentication, stderr flood, valid reply then
hang, memory exhaustion, forbidden child process, failed pending checkpoint,
successful synthetic effect, crash after a synthetic effect, a peer that never
reads stdin, and exit code 259. Additional checks revoke consent while a worker
is running, interrupt it with Stop, verify creation-bound termination, and prove
an unrelated sentinel process survives worker cleanup. These are synthetic
worker tests, not acceptance of the 12 frozen future real-app cases.

Initial compiler/test failures remain in the evidence directory. The first
compile exposed private method visibility; the initial exit assertion incorrectly
treated accessible metadata from an exited process as a live process. Native
termination checks now use the process handle's signaled state. An intermediate
fault reruns failed because fixture startup records were absent; a stricter run
also caught the aborting memory fixture timing out rather than proving memory
rejection. Cold-copy startup allowances were increased for protocol/resource
cases, failure-class assertions were tightened, and the memory fixture now uses
fallible allocation to verify rejection without the abort/crash-reporting path.
The hang cases retain two-second deadlines; the blocked-writer case uses five
seconds, and all initial fault calls must return within sixteen seconds including
cleanup. These changes affect test qualification, not production timeout limits.
Failed logs are retained separately from later qualification. Final commands, outcomes and exact
artifact/source identities are recorded in worker-result.json.

The optimized worker is qualified separately outside the checkout. This does
not rebuild/relabel the accepted Stage 3 desktop, update the Stage 0 installer,
or establish installation acceptance. Earlier JavaScript/Python results belong
to the foundation report; they are not newly executed worker acceptance cases.

Final native workspace regression: **423 passed, 0 failed, 7 ignored**. Five
ignored cases are historical; the explicit fault matrix and outside-checkout
worker qualification account for the other two and are executed separately.
Strict `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
The frozen register audit still validates 58 requirements, 17 source routes and
15 frozen Stage 0 cases. Existing agent AppContainer network/read/write checks,
DPAPI tests and Job memory/termination tests pass in the workspace regression.

The optimized x64 worker is 249,856 bytes with SHA-256
`831087c0ba746178f85082518d87e26d56f19fabf4945de7eb15c371322b77dc`.
Static PE inspection finds ASLR and DEP flags, no Guard CF flag and no
certificate table. Signing/compiler-hardening acceptance remains open. Selected
test-only marker strings are absent. These static checks supplement execution
tests; they are not a security audit. The exact outside copy and manifest are
recorded in [worker-artifact.json](../EVIDENCE/stage-4/worker-artifact.json).

## Next Stage 4 gate

Implement persisted native private-app/document settings and fresh target
admission before any content observation. Then qualify one exact editor/version
using dedicated disposable documents: bind the actual process/window/document,
open/inspect/edit/save/close normally, independently verify saved bytes, preserve
unsaved/newer work and handle dialogs/restarts conservatively. Integrate durable
pending/receipt recovery and the native coordinator only after these gates pass.
Spreadsheet qualification follows independently. No Stage 5/6 expansion, Laya
activation, Docker execution or claim of general desktop automation is made here.
