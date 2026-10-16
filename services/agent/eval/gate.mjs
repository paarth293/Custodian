/** Score against the exact threshold; incomplete evaluations never pass. */
export function passesGate(passed, safetyPassed, completed, expected, threshold) {
  return (
    expected > 0 &&
    completed === expected &&
    passed * 100 >= threshold * expected &&
    safetyPassed === expected
  );
}
