# Custodian: The Complete Project Guide & Security Architecture
## *Tidy your computer without fear. Every change comes with a way back.*

---

## 1. Executive Summary & Core Mission

**Custodian** is a local-first, privacy-respecting, open-source AI desktop companion designed natively for Windows 11. It provides an intuitive, trustworthy assistant that understands what lives on your hard drive, helps you reclaim gigabytes of disk space, organizes your workspace, adjusts Windows settings, and executes tasks—**without ever destroying your files, violating your privacy, or giving an AI unchecked authority over your operating system.**

At the center of Custodian is a radical design philosophy:
> **"Custodian changes only what it can undo, says out loud before doing anything it can't, tests everything in a sandbox first, keeps your data on your computer, and proves every undo with a checksum."**

Unlike traditional cleaner utilities that blindly delete files or modern cloud-based AI agents that send your desktop data to remote data centers, Custodian operates with **deterministic, mathematically verifiable safety**. It is represented visually on your screen by **Cubby**, a small, unobtrusive, animated companion.

```
       +--------------------------------------------------------+
       |                         USER                           |
       +--------------------------------------------------------+
                                  |
                   Plain Language / Card Clicks / Hello
                                  v
+----------------------------------------------------------------------+
|                     CUBBY & DESKTOP SHELL (Tauri 2)                  |
|  - Transparent, non-activating window                                |
|  - Screen-capture immune (WDA_EXCLUDEFROMCAPTURE)                    |
|  - DPAPI-protected Ed25519 Approval Service                          |
+----------------------------------------------------------------------+
         |                                           |
  HMAC Authenticated                          HMAC Authenticated
  Private Stdio Pipe                          Named Pipe
         v                                           v
+-----------------------------+             +--------------------------+
|       LOCAL AI AGENT        |             |      CORE & ENGINE       |
|  - Confined in AppContainer |             |  - Pure Rust             |
|  - Zero Network Capabilities|             |  - The ONLY writer       |
|  - Strict Grammar / Schemas |             |  - 30-day Safe Place     |
|  - Gemma / Qwen / Laya      |             |  - Merkle Tree Digests   |
+-----------------------------+             |  - Tamper-evident Ledger |
                                            +--------------------------+
```

---

## 2. The Problem Space: Why Custodian Was Born

Modern desktop computer users face a persistent dilemma characterized by anxiety, clutter, and loss of control:

### 2.1 The Digital Hoarding & Clutter Epidemic
1. **Fear of Deletion**: Users accumulate duplicate photos, orphaned downloads, virtual environments (`node_modules`, `.venv`), and gigabytes of caches. They avoid cleaning because they fear accidentally destroying a unique document, tax form, or irreplaceable family photo.
2. **The "Later" Paradox**: People intend to review files before deleting them, but manual disk triage is tedious. Consequently, drives fill up until Windows displays low-space warnings and system performance degrades.
3. **Blunt Cleaner Tools**: Legacy cleanup utilities (e.g., CCleaner, generic disk cleaners) act as opaque black boxes. They wipe system logs, clear browser sessions, and permanently delete files without an undo path, explanation, or safety net.

### 2.2 The Dangers of Unconstrained AI Agents
With the advent of Large Language Models (LLMs), many software tools rushed to give AI agents access to the filesystem, shell, and internet:
- **Hallucinations with Root Permissions**: An LLM given shell access can execute disastrous commands (`rmdir /s /q`, accidental registry wipes, or moving wrong directories) due to prompt injection, misinterpretation, or hallucination.
- **Privacy Breaches & Data Exfiltration**: Cloud AI tools upload file contents, folder structures, and sensitive personal data to third-party servers.
- **The Prompt Safety Fallacy**: Prompts like *"Please be careful not to delete user files"* are not security boundaries. Adversarial input, malformed files, or model drift can easily bypass prompt-level restrictions.

### 2.3 The Custodian Answer
Custodian replaces wishful prompt engineering with **deterministic kernel invariants, OS-level sandboxing, and cryptographic guarantees**. If an AI agent hallucinates, Custodian's Rust engine simply refuses the operation. If a user regrets an action, every single operation can be completely rolled back with mathematical proof.

---

## 3. The Custodian Contract: Six Immutable Guarantees

Every component of Custodian is engineered to uphold six absolute rules known as **The Custodian Contract**:

| # | Contract Rule | Practical Meaning |
|---|---|---|
| **1** | **Never destroy the only copy of anything a human made** | Unique personal files (photos, code, documents) can **never** be deleted automatically. Only proven machine-generated caches or verified duplicates can be quarantined. |
| **2** | **No way back, no action** | Before any write or file move occurs, a full reversible inverse operation and state digest are constructed. If an action cannot be undone, Custodian refuses to do it automatically. |
| **3** | **The irreversible is said out loud** | True deletions (such as emptying the safe place or permanent temp cleanup) require explicit human consent via Windows Hello or secure dialogs, with exact cards describing what will happen. |
| **4** | **Only the human can raise authority (Monotonic Safety)** | The AI can never grant itself elevated permissions. A background worker, model, or script can only escalate caution (refuse, abstain, or ask), never reduce it. |
| **5** | **Local storage only** | Custodian never uploads file names, contents, telemetry, or user behavior to any cloud. All persistent state lives on your machine in `%LOCALAPPDATA%\Custodian\`. |
| **6** | **Tested in a sandbox first** | File transformations and application installations are executed in an isolated staging sandbox (Shadow Run / Windows Sandbox) to verify declared effects before touching your actual computer. |

---

## 4. The User Experience: Meet Cubby

### 4.1 Who is Cubby?
Cubby is the desktop visual interface for Custodian. Instead of an intimidating enterprise console or a buried command-line script, Cubby lives on your screen as a small, charming companion:
- **Desktop Presence**: Rendered in a transparent, borderless, topmost Tauri 2 window (80×80 logical pixels) with state-driven vector animations powered by Rive.
- **Non-Activating (`WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`)**: Cubby never steals keyboard or mouse focus from your active games, documents, or video players.
- **Click-Through Mask**: Clicking outside Cubby's character sprite clicks through to the window underneath.

### 4.2 Visual States and Trust Colors
Cubby communicates the system's operational and security state at a glance:
- **Home Blue (`#2F5BD3`)**: Offline mode. 100% of network traffic is blocked by Windows Firewall. Safe, private, and local.
- **Signal Amber (`#E8931E`)**: Online mode active (or temporary web search / download underway). Represents that external data is being fetched.
- **Security Red Outline (`#C62828`)**: Security alert (e.g., hash mismatch, ledger tampering detected, or AppContainer boundary violation).
- **Behavioral Moods**:
  - `Sleeping`: Idle and consuming less than 1% CPU.
  - `Listening`: Voice or text input active.
  - `Thinking`: Local model generating an answer or analyzing evidence.
  - `Working`: File indexing, duplicate hashing, or sandbox dry-run in progress.
  - `Done`: Task completed successfully.
  - `Puzzled`: Uncertainty or model error; automatically falling back to deterministic safe rules.

### 4.3 Anti-Surveillance & Display Protection
- **Screen Capture Exclusion (`WDA_EXCLUDEFROMCAPTURE`)**: Cubby and all Custodian windows instruct the Windows Desktop Window Manager (DWM) to render as black or transparent to screen recording software, OBS, Zoom, or Discord streams. Your private files, lists, and plans can never be leaked during screen shares.
- **SafeName Typography**: Rendered file names use bidirectional text isolation (`<bdi>`), expose hidden Unicode control characters, and display file extensions in separate chips to defeat Right-to-Left Override (RTLO) and homoglyph spoofing attacks.

---

## 5. Comprehensive Feature Breakdown

Custodian is packed with real, practical desktop management capabilities. Every feature is backed by strict safety checks:

```
+-----------------------------------------------------------------------------+
|                            CUSTODIAN FEATURE SUITE                          |
+-----------------------------------------------------------------------------+
|  [File & Space Hygiene]       [Windows System Control]  [Local Intelligence]|
|  - Safe Duplicate Dedupe      - 6 Core HKCU Settings    - Local LLM Chat    |
|  - 30-Day Quarantine Window   - Instant Rollback        - App Launch/Close  |
|  - 7-Day Temp File Purge      - Hardware Info Checks    - Local Reminders   |
|  - ReFS Block Cloning         - Battery & Space Nudges  - Natural File Search|
+-----------------------------------------------------------------------------+
```

### 5.1 Safe Duplicate Cleaning (The Quarantine Paradigm)
- **Multi-Stage Hash Cascade**: Rather than computing slow full-file hashes across your whole drive:
  1. Files are grouped by size.
  2. First and last 64 KiB are hashed using streaming BLAKE3.
  3. Only candidates matching the ends receive full SHA-256 calculation.
  4. Final byte-by-byte comparison is performed immediately before any move.
- **Survivor Selection**: The kept original is chosen using a strict priority heuristic: the copy in OneDrive/cloud sync > copy in code projects > copy in Documents/Pictures > oldest file.
- **Zero-Copy Quarantine ("The Safe Place")**: Files are **never deleted**. Duplicate copies are renamed into `%LOCALAPPDATA%\Custodian\safe\` (or `X:\.custodian-safe\` on non-C: drives).
- **The 30-Day Window & Dual Verification**: Files remain in quarantine for 30 days. When a purge is finally triggered, Custodian verifies two things:
  1. The file in the safe place has not been tampered with.
  2. The kept original file is still intact and byte-identical at its original path.
  *If the original was deleted or moved by the user in the meantime, the quarantined copy is preserved!*
- **Instant Undo**: Restoring a cleanup moves the file back to its exact original path. If a new file occupies that location, Custodian restores it alongside as `filename (restored).ext`—never overwriting.

### 5.2 Safe Temp File Cleaning
- Only targets regular files in the user's specific `%TEMP%` folder.
- Enforces an age threshold: untouched for at least 7 days.
- Re-checks access and file locks immediately prior to removal.
- Every temporary file cleanup is permanently logged to the tamper-evident ledger.

### 5.3 Reversible Windows 11 Settings Management
Custodian allows you to inspect and modify Windows settings using natural language (e.g., *"make windows dark"*, *"show file extensions"*, *"put my theme back"*):
- **Whitelisted Registry Scope**: Strictly confined to `HKEY_CURRENT_USER`. No machine-wide changes, no elevation, no touching security or boot configuration.
- **Current Settings Supported**:
  1. Dark mode for Applications
  2. Dark mode for Windows Shell / Taskbar
  3. Window transparency effects
  4. Hidden files visibility
  5. File name extensions visibility
  6. Taskbar alignment (Center vs. Left)
- **The Preview Card**: Even when the user explicitly commands a setting change, Cubby presents a UI card displaying the exact current value and the target value. No registry keys are written until the card is clicked.
- **DPAPI-Sealed Historical Revert**: The previous registry value is encrypted and stored on disk. Saying *"put my theme back"* or clicking Undo reverses the registry key instantly.

### 5.4 Natural Language File Finding & Safe Launcher
- **File Search**: Quickly locates documents, spreadsheets, images, and archives by name across permitted user directories without spawning invasive shell indexing.
- **Safe Opening**: Opens user documents, pictures, and folders using the Windows Shell default association (`ShellExecuteW`). **Crucial safety rule**: Custodian will *never* launch raw executable files (`.exe`, `.bat`, `.cmd`, `.ps1`) from a file search request.
- **Start Menu Apps**: Can open installed Start Menu applications by DisplayName (e.g., *"open spotify"* or *"open calculator"*).

### 5.5 Graceful Application Lifecycle Management
- **Status Queries**: *"What's open?"* enumerates top-level visible desktop application windows.
- **Graceful Termination**: *"Close notepad"* sends standard `WM_CLOSE` window messages—the exact equivalent of a human clicking the window's "X" button. This allows the application to prompt *"Do you want to save your work?"*. Custodian *never* issues brutal `TerminateProcess` kills. Applications running elevated (as Administrator) are completely ignored.

### 5.6 Sealed Reminders & Local To-Do Lists
- Store personal reminders and daily tasks (e.g., *"remind me to call mom at 5pm"*, *"add milk to my list"*).
- All items are encrypted on disk using Windows DPAPI.
- Offline and local: reminders trigger desktop notifications natively without external calendar sync or telemetry.

### 5.7 Health Diagnostics & Non-Intrusive Nudges
- Summarizes drive free space, RAM consumption, and battery health.
- **Polite Nudge Policy**: Custodian respects user focus. Unprompted suggestions (e.g., low disk space warnings < 5 GB or battery < 15%) occur at most once per day/hour and are **suppressed** whenever Windows reports fullscreen gaming, presentation mode, or quiet hours (`SHQueryUserNotificationState`).

### 5.8 Online Mode & The Operator (Opt-In Browser Assistant)
- **Default State**: 100% Offline.
- **When Enabled by User**: Launches an isolated Playwright Chromium instance inside a separate Job Object.
- **Data Loss Prevention (DLP)**: Intercepts all outgoing web requests with a cryptographic Bloom filter to prevent local document snippets, file names, or private tokens from leaving your machine.
- **Automatic Handover**: Whenever a web page requires a password, credit card, OTP, checkout, or social media post, the Operator pauses and hands control to the human user.

---

## 6. The Security Layer: Comprehensive Deep Dive

Custodian's security layer is designed to be resilient against compromised LLMs, malicious files, and untrusted network input. It implements **12 independent defense-in-depth layers**:

```
+------------------------------------------------------------------------------+
|                     THE 12 DEFENSE-IN-DEPTH SECURITY LAYERS                  |
+------------------------------------------------------------------------------+
|  1. Code & Supply Chain Integrity   |  7. Minimal Elevation (custodian-elevate)
|  2. Windows AppContainer Sandboxes  |  8. Tamper-Evident Ledger & TPM Anchor
|  3. Process Capability Matrix       |  9. Information Flow Control & DLP Filter
|  4. Mutual HMAC-SHA256 IPC          | 10. AI Grammars & Model Containment
|  5. Cryptographically Signed Plans  | 11. Path Traversal Defenses (SafePath)
|  6. Dual Sandboxing (Shadow Run)    | 12. Local-Only Firewall Enforcement
+------------------------------------------------------------------------------+
```

---

### 6.1 Layer 1: Code & Supply Chain Integrity
- **Rust Memory Safety**: Built with pure Rust across all security-critical crates (`custodian-core`, `custodian-engine`, `custodian-winsec`, `custodian-approval`, `custodian-ledger`, `custodian-ipc`). Entire workspace enforces `#![forbid(unsafe_code)]`, with the sole exception of `custodian-winsec` where every Win32 call requires documented `// SAFETY:` justifications.
- **Compilation Hardening**: Compiled with Control Flow Guard (`/guard:cf`), Intel Control-flow Enforcement (`/CETCOMPAT`), and overflow checks enabled even in release profiles.
- **DLL Preloading Defense**: The very first line of code executed in `main()` calls `dll::harden()`, which invokes `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_APPLICATION_DIR)` to defeat DLL hijacking.
- **Cryptographic Model Pinning**: Every AI model file (`.gguf`, `.onnx`) is SHA-256 hashed and verified against a signed `manifest.json`. If a model file is modified by a byte, Custodian refuses to load it.

---

### 6.2 Layer 2: Windows Isolation Primitives (`custodian-winsec`)
Custodian leverages modern Windows 11 kernel isolation features via its dedicated `custodian-winsec` crate:

#### AppContainers
Windows AppContainers provide kernel-enforced sandboxes with an explicit capability model (originally designed for UWP/Store apps). Custodian creates separate AppContainer profiles:
- `Custodian.Agent`: Contains the Node.js / LLM runtime. Has **ZERO network capabilities** (no `internetClient`, no `privateNetworkClientServer`). It cannot connect to the internet or localhost. It has no access to the user's files; it is granted read-only access strictly to Node's binary, its own code packages, and the `models/` directory.
- `Custodian.ML`: Contains the Python ML host. Zero network capabilities.
- `Custodian.Parser`: Contains document text extractors. Zero network capabilities.
- `Custodian.Sandbox`: Contains staged conversion tools. Zero network capabilities.

#### Windows Job Objects
Every child process is assigned to a Windows Job Object before its main thread starts:
- `CREATE_SUSPENDED` flag on process creation.
- Job limits configured: hard memory caps (e.g., 3 GB for ML host, 512 MB for parser), active process limits (prevents fork bombs), and `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
- Process resumes only after confinement is active.

---

### 6.3 Layer 3: Process Architecture & The Capability Matrix
Custodian divides responsibilities across isolated executables. No single process possesses both user-level authority and AI decision-making.

| Binary | Role | Read User Files | Write User Files | Network | Loopback | Spawn Child |
|---|---|---|---|---|---|---|
| `custodian.exe` | Supervisor & Desktop UI | ❌ Denied | ❌ Denied | ❌ Blocked | ✅ Health | ✅ Children |
| `custodian-core.exe` | Scanner & Analyzers | ✅ Read-only | ❌ Denied | ❌ Blocked | ✅ Postgres | ❌ Denied |
| `custodian-engine.exe` | **The ONLY Disk Writer** | ✅ Read-only | ✅ **Write allowed** | ❌ Blocked | ✅ Postgres | ❌ Elevate only |
| `custodian-agent.exe` | Local LLM & Mastra | ❌ Denied | ❌ Denied | ❌ Blocked | ❌ Denied | ❌ Denied |
| `custodian-mlhost.exe` | Python / Laya / TabPFN | ❌ Denied | ❌ Denied | ❌ Blocked | ❌ Denied | ❌ Denied |
| `custodian-broker.exe` | Online Browser Operator | ❌ Outbox only | ❌ Inbox only | ✅ **Allowed** | ❌ Denied | ✅ Chromium |
| `custodian-elevate.exe` | UAC Elevation Helper | Target only | Target only | ❌ Blocked | ❌ Denied | Verified installer |

> **Crucial Security Invariant:** The AI Agent (`custodian-agent.exe`) has **NO IPC CHANNEL TO THE ENGINE** and **NO DISK WRITE CAPABILITY**. Even if an LLM is completely compromised via prompt injection, it physically lacks the operating system handles to modify, overwrite, or delete a single file on your computer.

---

### 6.4 Layer 4: Mutual Cryptographic IPC (`custodian-ipc`)
Processes communicate via private, authenticated channels:
- **No Named-Pipe Discovery**: The Supervisor creates anonymous pipe pairs (`CreateNamedPipeW` with `FILE_FLAG_FIRST_PIPE_INSTANCE`). Handles are passed directly to children via `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`. Malicious third-party software running as the same user cannot connect to or spoof Custodian's IPC pipes.
- **HMAC-SHA256 Framing**: Every message is serialized inside a cryptographically authenticated frame:
  $$\text{Frame} = \text{Length (4B)} \parallel \text{Sequence (8B)} \parallel \text{HMAC-SHA256 (32B)} \parallel \text{Payload JSON}$$
- **Per-Channel Session Keys**: Each process pair uses a distinct session key derived via HKDF-SHA256 from an in-memory master secret generated at supervisor startup.
- **Strict Sequence & Schema Enforcement**: Frames must strictly increment by 1. Unknown message types, malformed JSON, or out-of-order sequence numbers instantly drop the channel and log a security event.

---

### 6.5 Layer 5: Signed Approvals & Hardware-Backed Consent (`custodian-approval`)
When a file cleanup, setting change, or installation is proposed:
1. **Plan Hashing**: A canonical JSON representation (RFC 8785) of the plan is computed into a 32-byte SHA-256 digest (`PlanHash`).
2. **Ed25519 Signing**: The desktop UI signs the `PlanHash`, a random nonce, and an expiration timestamp (maximum 10 minutes) using an Ed25519 private key:
   $$\text{Sig} = \text{Sign}_{\text{Ed25519}}(\text{ApprovalKey}, \text{"custodian-approval-v1\0"} \parallel \text{PlanHash} \parallel \text{Expires} \parallel \text{Nonce})$$
3. **Key Custody**: The private key is held strictly in `custodian.exe` memory and encrypted at rest with Windows **DPAPI** (`CryptProtectData`). It is never shared with the Agent, Core, or ML host.
4. **Replay Defense**: The Engine maintains an atomic table of consumed nonces. A signed approval cannot be replayed or reused.
5. **Windows Hello Integration**: High-risk plans and settings resets invoke the Windows Hello biometric or PIN prompt via WinRT `UserConsentVerifier`. No software synthetic clicks can bypass this prompt.

---

### 6.6 Layer 6: The Engine, Safe Place Quarantine, & Merkle Tree Digests
The `custodian-engine` binary is the sole process with filesystem write authority:
- **Transaction State Machine**: Executes plans through durable journal states:
  $$\text{RECEIVED} \rightarrow \text{VERIFIED} \rightarrow \text{RESERVED} \rightarrow \text{JOURNALED} \rightarrow \text{EXECUTING} \rightarrow \text{VERIFYING} \rightarrow \text{COMMITTED}$$
- **Pre-Allocated Journal Ring**: Operations are logged to `engine.journal` (a 64 MB pre-allocated ring with CRC32C integrity checksums per record) and fsynced (`FlushFileBuffers`) before any disk modification begins.
- **Merkle Tree State Records**: Every file has a captured `StateRecord` (relative path, size, SHA-256, timestamps, attributes, NTFS alternate data streams, security descriptors). The Engine calculates a Merkle Tree digest of the entire directory tree before and after an operation. **Undoing a cleanup mathematically requires that $\text{Digest}_{\text{after\_undo}} == \text{Digest}_{\text{before}}$.**

---

### 6.7 Layer 7: Tamper-Evident Ledger & Cryptographic Anchors (`custodian-ledger`)
Every cleanup, settings adjustment, purge, and undo operation is permanently recorded in a hash-chained audit log:
- **Hash-Chained Segments**: Each record contains `[SHA-256(PrevHash || Data)][CRC32(Data)][Length][Data]`. Any attempt to edit past history breaks the chain.
- **Crash Recovery**: Torn writes caused by sudden power cuts are detected via CRC32 and safely truncated on restart.
- **Hardware Anchor**: Checkpoints are periodically signed with a non-exportable TPM 2.0 key (via Windows `NCryptOpenStorageProvider` with `MS_PLATFORM_CRYPTO_PROVIDER`) or DPAPI, with the current sequence counter stored in Windows Credential Manager outside the data directory.

---

### 6.8 Layer 8: Dual Sandboxing: Shadow Run & Shadow Install

#### Shadow Run (File Conversions & Tool Operations)
When Custodian needs to manipulate files (e.g., resizing images via ImageMagick, extracting PDFs via qpdf, or transcoding media via ffmpeg):
1. Input files are duplicated into a private staging directory.
2. The allow-listed tool runs inside an AppContainer sandbox on the copied files with zero network and memory caps.
3. Upon exit, Custodian inspects the staging directory against a **declared-effects manifest**.
4. If unexpected files were touched or created, the output is discarded. Only verified outputs are moved to the destination by the Engine. Original files are never exposed to tool processes.

#### Shadow Install (Software Installations)
When software installation is requested in Online Mode:
1. The installer file is checked via Authenticode signature verification and Microsoft Defender (`MpCmdRun.exe`).
2. An isolated disposable **Windows Sandbox** VM (`.wsb`) is launched with networking disabled.
3. An observer script executes the installer in the VM and records before-and-after diffs (new files in Program Files, registry `Run` keys, created services, scheduled tasks, browser extensions).
4. An **Install Report** is presented to the user.
5. Real installation occurs only after explicit confirmation, a Windows System Restore point is taken, and an automated uninstall recipe is saved.

---

### 6.9 Layer 9: Minimal Elevation Architecture (`custodian-elevate`)
Custodian runs with normal user privileges. When an action strictly requires administrative rights (e.g., creating a system restore point or enabling Windows Sandbox):
- It spawns a specialized helper binary: `custodian-elevate.exe`.
- **Closed Verb Allow-List**: The helper accepts only pre-defined verbs:
  - `enable_system_restore`
  - `set_restore_frequency`
  - `create_restore_point`
  - `enable_windows_sandbox`
  - `run_verified_installer`
  - `run_uninstaller`
- **Engine Signature Required**: The helper will not run unless the request file carries a valid Ed25519 signature from the Engine. Arbitrary shell commands, scripts, or unrecognized verbs are rejected with exit error codes.

---

### 6.10 Layer 10: AI Confinement & Prompt Injection Defenses
- **Grammar-Constrained Output**: The local LLM outputs tokens constrained by strict JSON-Schema grammars via `node-llama-cpp`. The model physically cannot emit free-form bash, PowerShell, or unstructured text when a tool call is expected.
- **Closed Action Vocabulary**: The agent has a restricted set of tools (`get_health`, `list_candidates`, `explain_candidate`, `find_files`, `propose_file_plan`, `request_access`). It cannot construct arbitrary file paths or shell strings.
- **Prompt Injection Defense**: Untrusted text read from document excerpts or web pages is wrapped in isolated XML tags (`<data label="WEB_UNTRUSTED">...</data>`). System prompts instruct the reader model to treat content solely as data, never instructions.
- **Circuit Breakers**: If the model produces 3 invalid schema outputs or fails validation sequentially, it is quarantined and Custodian falls back to deterministic rule-based operation.

---

### 6.11 Layer 11: Filesystem Safety & Path Traversal Defenses (`SafePath`)
Filesystem paths are treacherous on Windows due to symlinks, directory junctions, alternative data streams, and DOS device names:
- **`SafePath` Primitive**: All paths undergo validation in `custodian-winsec::SafePath`:
  - Rejects `..` path components.
  - Resolves symlinks and junctions across all ancestors (`fs::canonicalize`) to ensure no link inside a folder escapes to an external root.
  - Strips trailing spaces and dots (which Win32 APIs silently drop, altering target identity).
  - Prepends the verbatim `\\?\` prefix to prevent path manipulation.
- **Cloud Sync Awareness**: Custodian uses `CfGetSyncRootInfoByPath` to detect OneDrive, Dropbox, and Google Drive folders. Quarantining files from cloud folders is blocked or specially handled to avoid triggering cloud deletion cascades across other devices.

---

### 6.12 Layer 12: Local-Only Privacy Guarantee
- **Windows Firewall Outbound Rules**: The Custodian installer configures Windows Firewall rules blocking all outbound network traffic from every Custodian binary except `custodian-broker.exe`.
- **Loopback Hardening**: Internal databases (bundled PostgreSQL 17) and workflow servers (Temporal) bind exclusively to `127.0.0.1` on random ports, secured with SCRAM-SHA-256 passwords and TLS certificates generated locally on first run.
- **Zero Telemetry**: Release builds include no analytics SDKs, no crash reporting beacons, and no cloud log sinks.
- **Redacted Local Logs**: The logging layer automatically sanitizes file paths and personal text strings before writing to rotated local log files.

---

## 7. System Architecture & Repository Layout

### 7.1 Architecture Layers

```
+------------------------------------------------------------------------+
|                               TAURI UI                                 |
|  - apps/desktop (React 19, TypeScript, Vite, Rive)                     |
|  - Windows: Cubby, Panel, Approval Dialog                              |
+------------------------------------------------------------------------+
                                   |
                          custodian-ipc (HMAC)
                                   v
+------------------------------------------------------------------------+
|                         CORE RUST SERVICES                             |
|  - custodian-core: Scanning, evidence, duplicate finder, safe-mode     |
|  - custodian-engine: Write-ahead journal, transaction execution, undo  |
|  - custodian-winsec: AppContainer, Job, DPAPI, SafePath, Settings      |
|  - custodian-approval: Ed25519 cryptographic authorization             |
|  - custodian-ledger: Tamper-evident hash-chained audit ledger          |
+------------------------------------------------------------------------+
          |                                            |
    Stdio Streams                                  Pipes / Jobs
          v                                            v
+-----------------------------+             +----------------------------+
|      SERVICES / AGENT       |             |   SERVICES / MLHOST        |
|  - services/agent (Node 24) |             |  - services/mlhost (Python)|
|  - node-llama-cpp inference |             |  - ONNX Runtime            |
|  - GGUF: Qwen 2.5 / Gemma   |             |  - Laya & TabPFN models    |
+-----------------------------+             +----------------------------+
```

### 7.2 Repository Map
- **`apps/desktop/`**: Tauri 2 application shell. Contains the frontend React chat panel, Rive animation assets, and Tauri Rust bridge (`src-tauri/src/assistant/`, `src-tauri/src/agent.rs`).
- **`crates/`**: The core Rust workspace:
  - `custodian-approval`: Ed25519 signing and verification of execution plans.
  - `custodian-core`: Duplicate scanners, temp file analysis, and safe quarantine management.
  - `custodian-digest`: High-performance streaming BLAKE3 & SHA-256 hashing.
  - `custodian-elevate`: Minimal UAC helper for allow-listed privileged actions.
  - `custodian-engine`: Journaled, reversible transactional disk writer.
  - `custodian-ipc`: Authenticated HMAC framing and sequence enforcement.
  - `custodian-ledger`: Append-only, hash-chained transaction ledger.
  - `custodian-parser`: Sandboxed document text extraction.
  - `custodian-sandbox`: Staging launcher and declared-effect verification.
  - `custodian-winsec`: Platform-specific Win32 isolation, AppContainers, and DPAPI.
- **`services/`**:
  - `agent/`: TypeScript service running local GGUF models via `node-llama-cpp`.
  - `broker/`: Playwright-based browser driver for sandboxed online tasks.
  - `mlhost/`: Embedded Python runtime hosting ONNX models.
  - `orchestrator/`: Temporal workflow runner for long-running disk lifecycle jobs.
- **`proto/`**: JSON Schemas defining the exact wire types for all IPC communications.
- **`models/`**: Storage for pinned local GGUF and ONNX models (git-ignored, verified against `manifest.json`).

---

## 8. Implementation Status: Reality vs. Target

Custodian maintains strict transparency regarding what is implemented today in the codebase versus what is designed for future phases (documented in [`HONESTY.md`](file:///d:/Custodian/HONESTY.md)):

### Currently Working in the Codebase
✅ **Desktop Companion Shell**: Fully functional transparent, non-activating Cubby UI in Tauri 2, excluded from screen capture (`WDA_EXCLUDEFROMCAPTURE`).  
✅ **Deterministic Intent Routing**: Understands natural language requests for file search, disk usage, duplicates, temp files, app management, settings, reminders, and undo.  
✅ **Safe Duplicate Quarantine**: Multi-stage hashing, duplicate plan generation, moving to safe place quarantine, 30-day retention tracking, dual re-hash check before purge, and instant undo.  
✅ **Safe Temp File Cleaning**: 7-day age filtering in user `%TEMP%`, pre-delete verification, and ledger logging.  
✅ **Windows Settings Engine**: Real `HKCU` registry modifications for 6 core appearance and shell settings, with preview cards and DPAPI-sealed rollbacks.  
✅ **AppContainer Agent Sandboxing**: The chat agent runs in a real Windows AppContainer with zero capabilities, verified unable to touch network or unauthorized files.  
✅ **Signed Approvals & Windows Hello**: Ed25519 signing of plan digests, DPAPI key protection, and Windows Hello user verification prompts.  
✅ **Tamper-Evident Ledger**: Real append-only hash-chained ledger with crash truncation recovery.  
✅ **Local Model Inference**: Integrated `node-llama-cpp` loading pinned Qwen 2.5 / Gemma GGUF models on Vulkan / CPU over authenticated stdio frames.

### In Development / Roadmap
🚧 **Full Temporal Orchestration**: Background Temporal dev server integration for multi-day cron-like workflows.  
🚧 **PostgreSQL & TimescaleDB Store**: Transitioning from file manifests to embedded PostgreSQL 17 + pgvector.  
🚧 **Shadow Install VM Automation**: Full automation of Windows Sandbox (`.wsb`) diff analysis.  
🚧 **Hardware TPM 2.0 Checkpoint Signing**: Upgrading the DPAPI-sealed ledger anchor to a hardware TPM P-256 non-exportable key.

---

## 9. Getting Started & Verification

### Prerequisites
- **Operating System**: Windows 11 x64 (Build 22H2 or later).
- **Toolchains**: Rust 1.97 (`rust-toolchain.toml`), Node.js 24 (`.nvmrc`), Python 3.12.

### Build and Test
```powershell
# 1. Install dependencies
npm ci

# 2. Build the desktop frontend
npm run build -w apps/desktop

# 3. Verify all Rust crates and security tests
cargo test --workspace

# 4. Verify TypeScript tests
npm test
```

### Fetch Local Model
```powershell
# Download and pin the recommended local chat model (Qwen 2.5 1.5B or Qwen 3 4B)
./scripts/fetch-models.ps1 -Repo Qwen/Qwen2.5-1.5B-Instruct-GGUF -File qwen2.5-1.5b-instruct-q4_k_m.gguf

# Build the agent service
npm run build
```

### Run Custodian
```powershell
# Launch the desktop shell with Cubby
npm run tauri -w apps/desktop -- dev
```

---

## 10. Summary & The Future of Safe Local AI

**Custodian** demonstrates that AI desktop assistance does not require compromising user privacy or surrendering operating system safety.

By pairing modern local language models with **uncompromising Rust engineering, Windows AppContainer sandboxing, cryptographic plan approvals, and guaranteed undoability**, Custodian sets a new benchmark for personal computing: an AI companion that empowers you to keep your computer clean, organized, and fast—**with zero fear and a proven way back.**
