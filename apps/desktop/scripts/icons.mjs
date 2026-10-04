// Regenerates the desktop icon set from src-tauri/icons/custodian.svg and
// removes the mobile and extra sizes the Tauri CLI also emits but this
// desktop-only app does not bundle (see bundle.icon in tauri.conf.json).
import { execFileSync } from 'node:child_process';
import { rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const appDir = join(dirname(fileURLToPath(import.meta.url)), '..');
const iconsDir = join(appDir, 'src-tauri', 'icons');

execFileSync('npx', ['tauri', 'icon', join(iconsDir, 'custodian.svg'), '-o', iconsDir], {
  cwd: appDir,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

for (const unused of ['android', 'ios', '64x64.png']) {
  rmSync(join(iconsDir, unused), { recursive: true, force: true });
}
