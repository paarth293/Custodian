/** Per-task budget of online minutes. */
export class BudgetManager {
  constructor(public minutes = 60) {}

  get exhausted(): boolean {
    return this.minutes <= 0;
  }

  tick(): void {
    if (this.minutes > 0) this.minutes--;
  }
}
