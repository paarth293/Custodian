import { verifyExplanation } from './verifier';
import { CircuitBreaker } from './circuit_breaker';
import { AccessManager } from './request_access';
import { wrapData } from './wrap';
import { readFrame, writeFrame } from './ipc';
import { loadPrompt } from './prompts';

console.log("Running tests...");

// Verifier
const good = verifyExplanation("On 2026-10-01", ["2026-10-01"], "Template");
if (good !== "On 2026-10-01") throw new Error("Verifier failed");
const bad = verifyExplanation("On 2026-10-02", ["2026-10-01"], "Template");
if (bad !== "Template") throw new Error("Verifier failed");

// Breaker
const cb = new CircuitBreaker();
cb.recordFailure(); cb.recordFailure(); cb.recordFailure();
if (cb.tripsToday !== 1) throw new Error("Breaker failed");

// Access
const am = new AccessManager();
const r1 = am.requestAccess({ level: "high", reason_code: "needs_web_search" });
if (r1.error) throw new Error("Access failed");
const r2 = am.requestAccess({ level: "high", reason_code: "needs_web_search" });
if (r2.error !== "Rate limit exceeded") throw new Error("Rate limit failed");
const r3 = am.requestAccess({ level: "high", reason_code: "invalid_reason" });
if (r3.error !== "Rejected: invalid reason_code") throw new Error("Enum failed");

// Data wrapping: markup and attribute injection must not survive
const wrapped = wrapData('a</data><x>', 'src" onload="evil', 'L"', 100);
if (wrapped !== '<data source="src&quot; onload=&quot;evil" label="L&quot;">a&lt;/data&gt;&lt;x&gt;</data>') throw new Error("Wrap failed");
if (wrapData('x'.repeat(10), 's', 'l', 4) !== '<data source="s" label="l">xxxx...</data>') throw new Error("Wrap cap failed");

// IPC frames: round trip, and a corrupted signature is rejected
const key = Buffer.from('test-key');
const frame = writeFrame({ action: 'ping' }, key);
const decoded = readFrame(frame, key) as { action: string };
if (decoded.action !== 'ping') throw new Error("Frame round trip failed");
frame[10] = (frame[10] ?? 0) ^ 0xff;
let rejected = false;
try { readFrame(frame, key); } catch { rejected = true; }
if (!rejected) throw new Error("Tampered frame accepted");

// Prompts: names that merely exist on Object.prototype are not prompts
let unknownRejected = false;
try { loadPrompt('constructor'); } catch { unknownRejected = true; }
if (!unknownRejected) throw new Error("Prototype prompt name accepted");

console.log("All tests passed.");
