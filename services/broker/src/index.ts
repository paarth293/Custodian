export class BrowserBroker {
    profile: string = "isolated_profile";
    
    start(flags: string[]) {
        console.log("Broker started with flags: ", flags);
        if (!flags.includes('--no-sandbox')) {
            console.log("Chromium sandbox enabled");
        }
    }
    
    async performAction(action: string, _params: unknown) {
        const validActions = ['navigate', 'search', 'read', 'click', 'type', 'select', 'scroll', 'download', 'submit', 'handover'];
        if (!validActions.includes(action)) throw new Error("E_POLICY");
        return Action  + action +  executed;
    }
}
