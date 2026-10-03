import * as fs from 'fs';
import * as path from 'path';

export function loadPrompt(name: string) {
    const manifestPath = path.join(__dirname, '..', 'prompts', 'manifest.json');
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    if (!Object.prototype.hasOwnProperty.call(manifest, name)) throw new Error("sec.event: unknown prompt");
    return fs.readFileSync(path.join(__dirname, '..', 'prompts', name + '.md'), 'utf8');
}
