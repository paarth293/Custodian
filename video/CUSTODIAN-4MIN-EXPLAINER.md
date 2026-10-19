# Custodian — 4-minute product explainer

## Production status

This is the render-ready master script and shot list for a 240-second 16:9
explainer. It is based on the current repository, especially `README.md`,
`HONESTY.md`, and `docs/HACKATHON-DELIVERY-RECORD.md`. It intentionally presents
Custodian as a safety-first prototype rather than claiming unfinished features.

The current workspace has no source footage, logo exports, voice track, music,
or video encoder. The script is therefore complete and ready to render, but an
MP4 cannot be emitted in this environment until FFmpeg (or another approved
encoder) and, optionally, brand assets/audio are available.

## Creative direction

- **Tone:** premium, calm, confident, trustworthy; never “AI magic” hype.
- **Visual language:** deep navy background, electric cyan safety lines, warm
  amber approval states, white typography, restrained motion.
- **Pacing:** a fast cold open, then deliberate proof. Every claim is paired
  with a visible artifact: preview, approval, receipt, refusal, or undo.
- **Music:** low-volume minimal electronic pulse, 92–100 BPM; duck under VO.
- **Voice:** clear neutral English, 145–155 words per minute.
- **Captions:** always on; highlight “preview”, “approve”, “verify”, “undo”.

## Master timeline and narration

### 00:00–00:18 — Cold open: “Your computer should not be a gamble”

**Picture:** Rapid montage: cluttered file tree, a red “overwrite?” warning,
then a clean Custodian interface. Freeze on the line: **Tidy your computer
without fear. Every change comes with a way back.**

**Narration:** “Every computer collects clutter. But cleaning it should never
feel like handing control to a black box. Custodian is a local, safety-first AI
companion for Windows—built to help, explain, verify, and give you a way back.”

### 00:18–00:40 — The problem and the promise

**Picture:** Three cards animate in: *Too much to find*, *Too risky to change*,
*Too hard to undo*. Replace them with one flow: **Ask → Preview → Approve →
Verify → Undo**.

**Narration:** “The idea is simple: you ask in plain language, Custodian turns
that request into a bounded plan, shows the exact consequence, waits for your
approval, performs only the approved operation, and records evidence you can
inspect or undo. Model text never becomes authority.”

### 00:40–01:12 — Meet Cubby

**Picture:** Stylized desktop mockup: Cubby opens from the tray; chat request
appears: “Find the Cedar files, excluding private.” A scope chip shows the
permitted folder and exclusion.

**Narration:** “Cubby is the on-screen interface. Ask where a resume is, find
duplicates in Pictures, clean old temporary files, or check how much space is
available. For the demo, we use an isolated Cedar fixture: public files, a
private canary, and an instruction-like file that must not gain authority.”

### 01:12–01:48 — Measurement before mutation

**Picture:** A measured preview builds line by line: exact paths, file count,
bytes, exclusion, destination, recovery policy. The private canary stays behind
a locked boundary. Add a small caption: **No preview, no change.**

**Narration:** “Before anything changes, Custodian measures the selection. The
preview names the exact paths, count, size, exclusion rules, destination, and
recovery behavior. Private and protected locations are refused. The assistant
can suggest an action—but only the approved, validated plan can execute.”

### 01:48–02:22 — Approval and verified execution

**Picture:** Amber approval card with “Approve” and “Stop”. A native approval
moment is shown, followed by a progress rail: *copying → hashing → verified*.
Finish on a green receipt card with transaction ID and timestamps.

**Narration:** “When the plan is clear, you approve it. The operation is bounded
and reversible: originals can be preserved, the result is checked against the
source, and a receipt records what happened. Stop can end a scan or answer.
Nothing silently expands its scope.”

### 02:22–02:52 — Refusal is a feature

**Picture:** Split screen. Left: approved source changed after preview. Right:
destination conflict. Both turn red with precise messages: **Refused — stale
source** and **Refused — would overwrite**. Then show a partial-result report.

**Narration:** “Safety is also what Custodian refuses to do. If a source changes,
the destination conflicts, or the request is stale, it does not substitute a
different file or overwrite by surprise. It reports the conflict clearly,
leaving the decision with you.”

### 02:52–03:20 — Undo without erasing later work

**Picture:** Timeline receipt is opened. The user edits a verified output. Undo
is requested; Custodian detects the later edit and preserves it, showing an
explanation rather than destroying it.

**Narration:** “Undo is evidence-aware. If a verified output was edited later,
Custodian does not bulldoze that newer work. The receipt remains available, the
later edit is retained, and the system explains why a destructive undo was
refused.”

### 03:20–03:44 — How it is built

**Picture:** Clean architecture animation: Cubby / Tauri + React → typed IPC →
Rust core and supervision → local agent and model. Add badges: least privilege,
authenticated IPC, journaled operations, local-first.

**Narration:** “Under the interface is a deliberately layered system. The
desktop shell is Tauri and React. Rust handles supervision, permissions,
journaling, reversible file operations, and verification. TypeScript services
coordinate the local agent and workflows. Components communicate through
authenticated IPC, with least privilege at each boundary.”

### 03:44–04:00 — Honest close and call to action

**Picture:** Three proof words land one at a time: **Preview. Verify. Recover.**
End card: **Custodian — tidy your computer without fear.** Small footer:
“Open-source Windows prototype • safety-first, local, verifiable”.

**Narration:** “Custodian is not an unrestricted computer operator—and that is
the point. It is a growing, honest foundation for local automation where every
important action is visible, bounded, verified, and recoverable. Preview. Verify.
Recover. Custodian.”

## On-screen text rules

1. Never show claims such as “fully autonomous”, “ransomware-proof”, or “all
   file formats supported”.
2. Keep primary text under 42 characters per line and secondary text under 70.
3. Use amber only for a human decision and red only for a refusal or boundary.
4. Use real repository terminology: *receipt*, *stale*, *protected path*,
   *verified*, and *undo*.

## Render contract

- Canvas: 1920×1080, 30 fps, 240 seconds exactly.
- Video: H.264 High profile, yuv420p, 12–18 Mbps, faststart enabled.
- Audio: AAC 48 kHz stereo, −16 LUFS integrated, −1 dBTP ceiling.
- Captions: burned-in for the review copy; separate WebVTT for accessibility.
- Deliverables: `custodian-4min-explainer.mp4`, `custodian-4min-explainer.vtt`,
  and this script.
