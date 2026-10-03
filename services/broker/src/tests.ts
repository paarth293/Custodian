import { BrowserBroker } from './index';

async function run() {
    const broker = new BrowserBroker();
    broker.start(['--disable-gpu']);
    
    const res = await broker.performAction('navigate', { url: 'http://localhost' });
    if (res !== 'Action navigate executed') throw new Error("Failed");
    
    try {
        await broker.performAction('unknown', {});
        throw new Error("Should have failed");
    } catch(e) {
        console.log("Typed actions validated.");
    }
}
run();
