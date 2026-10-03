/** Consecutive invalid model outputs that trip the breaker. */
export const FAILURES_PER_TRIP = 3;
/** Trips per day after which the model is disabled. */
export const TRIPS_PER_DAY = 3;

/** Quarantines the model after repeated invalid outputs. */
export class CircuitBreaker {
  invalidCount = 0;
  tripsToday = 0;

  get disabled(): boolean {
    return this.tripsToday >= TRIPS_PER_DAY;
  }

  recordFailure(): void {
    this.invalidCount++;
    if (this.invalidCount >= FAILURES_PER_TRIP) {
      this.tripsToday++;
      this.invalidCount = 0;
      console.log('ui.event{puzzled, model_quarantined}');
    }
  }

  recordSuccess(): void {
    this.invalidCount = 0;
  }
}
