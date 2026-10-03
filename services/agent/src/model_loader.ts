import * as fs from 'node:fs';
import * as path from 'node:path';
import { matchesManifest, readManifest } from './integrity';

export const DEFAULT_MODELS_DIR = path.resolve(__dirname, '..', '..', '..', 'models');

/**
 * Verifies a model file against `models/manifest.json` before it is loaded.
 * Emits a `sec.event` and returns false on any mismatch (T-SUPPLY-01).
 */
export function loadModelWithVerification(
  modelName: string,
  modelsDir = DEFAULT_MODELS_DIR,
): boolean {
  const manifestPath = path.join(modelsDir, 'manifest.json');
  const modelPath = path.join(modelsDir, modelName);
  if (!fs.existsSync(manifestPath) || !fs.existsSync(modelPath)) return false;

  if (!matchesManifest(readManifest(manifestPath), modelName, fs.readFileSync(modelPath))) {
    console.error(`sec.event: model ${modelName} failed verification`);
    return false;
  }
  return true;
}
