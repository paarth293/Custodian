export class CircuitBreaker {
    invalidCount = 0;
    tripsToday = 0;

    recordFailure() {
        this.invalidCount++;
        if (this.invalidCount >= 3) {
            this.tripsToday++;
            this.invalidCount = 0;
            console.log("ui.event{puzzled, model_quarantined}");
            if (this.tripsToday >= 3) {
                console.log("Model disabled for today.");
            }
        }
    }

    recordSuccess() { 
        this.invalidCount = 0; 
    }
}
