import { getLlama, LlamaChatSession } from "node-llama-cpp";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

async function main() {
    // If the 4B model doesn't exist, this will fail. We can wrap it in a fallback logic.
    let modelPath = path.join(__dirname, "models", "qwen2.5-1.5b-instruct-q4_k_m.gguf");
    
    console.log(`Loading model from: ${modelPath}`);

    const t0 = performance.now();
    
    const llama = await getLlama({ build: 'never', gpu: false });
    const model = await llama.loadModel({
        modelPath: modelPath
    });
    const context = await model.createContext();
    const session = new LlamaChatSession({ contextSequence: context.getSequence() });
    
    const t1 = performance.now();
    console.log(`Time to load the model: ${(t1 - t0).toFixed(2)} ms`);

    const q = "Reply with exactly: Hello.";
    let firstTokenTime = null;
    let tokensGenerated = 0;

    console.log(`Sending prompt: "${q}"`);
    const t2 = performance.now();
    const response = await session.prompt(q, {
        maxTokens: 32,
        onToken(tokens) {
            if (!firstTokenTime) {
                firstTokenTime = performance.now();
            }
            tokensGenerated += tokens.length;
        }
    });
    const t3 = performance.now();

    const tToFirstToken = firstTokenTime ? (firstTokenTime - t2) : (t3 - t2);
    const totalGenTime = t3 - t2;
    
    console.log(`\n=== Results ===`);
    console.log(`Response: ${response}`);
    console.log(`Time until the first generated token: ${tToFirstToken.toFixed(2)} ms`);
    console.log(`Total generation time: ${totalGenTime.toFixed(2)} ms`);
    console.log(`Tokens generated: ${tokensGenerated}`);
    console.log(`Tokens generated per second: ${(tokensGenerated / (totalGenTime / 1000)).toFixed(2)}`);
}

main().catch(console.error);
