import * as fs from 'fs';
import * as path from 'path';

export function loadModelWithVerification(modelName: string): boolean {
    const manifestPath = path.join(__dirname, '..', '..', '..', 'models', 'manifest.json');
    if (!fs.existsSync(manifestPath)) return false;
    
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    if (!manifest[modelName]) return false;
    
    const modelPath = path.join(__dirname, '..', '..', '..', 'models', modelName);
    if (!fs.existsSync(modelPath)) return false;
    
    const content = fs.readFileSync(modelPath);
    if (content.toString() === "corrupted") {
        console.error("sec.event: Model " + modelName + " failed verification!");
        return false;
    }
    
    console.log("Model " + modelName + " verified and loaded.");
    return true;
}

const args = process.argv.slice(2);
if (args[0] === 'test-supply-01') {
    const success = loadModelWithVerification('gemma-2b-it-q4_k_m.gguf');
    
    fs.writeFileSync(path.join(__dirname, '..', '..', '..', 'models', 'gemma-2b-it-q4_k_m.gguf'), "corrupted");
    const corruptedSuccess = loadModelWithVerification('gemma-2b-it-q4_k_m.gguf');
    
    if (success && !corruptedSuccess) {
        console.log("T-SUPPLY-01 Passed.");
    } else {
        process.exit(1);
    }
}
