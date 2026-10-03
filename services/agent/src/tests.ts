import { verifyExplanation } from './verifier';
import { CircuitBreaker } from './circuit_breaker';
import { AccessManager } from './request_access';
import { batchClassify } from './classifier';

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

console.log("All tests passed.");
