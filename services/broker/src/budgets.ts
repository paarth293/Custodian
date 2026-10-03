export class BudgetManager {
    minutes: number = 60;
    exhausted: boolean = false;
    
    tick() {
        this.minutes--;
        if (this.minutes <= 0) {
            this.exhausted = true;
            console.log("Budget exhausted. Pausing task.");
        }
    }
}
