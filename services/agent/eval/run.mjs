// Measures how well a local model uses Cubby's tools: the right tool, the
// right arguments, correct answers, and no claims of deleting anything.
//
//   npm run build -w services/agent
//   node services/agent/eval/run.mjs <model file pinned in models/manifest.json>
//
// Desktop tools get fixed, realistic results, so only the model is measured.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { createLlamaEngine, findChatModel, DEFAULT_MODELS_DIR } = require('../dist/llm.js');
const { TOOLS } = require('../dist/tools.js');

const modelName = process.argv[2];
if (!modelName) {
  console.error('usage: node eval/run.mjs <model file>');
  process.exit(2);
}
const model = findChatModel(DEFAULT_MODELS_DIR, modelName);
if (!model) {
  console.error(`${modelName} is not a pinned GGUF model in ${DEFAULT_MODELS_DIR}`);
  process.exit(2);
}
const cases = JSON.parse(readFileSync(new URL('./cases.json', import.meta.url), 'utf8'));

/** What each desktop tool "returns" during the evaluation. */
function canned(name, args) {
  const shown = (text) => `Shown to the person: ${text}`;
  switch (name) {
    case 'check_disk_space':
      return shown('C: has 41 GB free of 476 GB. D: has 210 GB free of 931 GB.');
    case 'find_large_files':
      return shown(
        'The 3 biggest files: movie.mkv (4.2 GB), setup.exe (1.1 GB), photos.zip (800 MB).',
      );
    case 'find_duplicate_files':
      return shown(
        'Found 12 extra copies of 9 files (3.4 GB). A clean-up plan is waiting for their approval; nothing has moved.',
      );
    case 'find_files':
      return shown(
        `2 files match "${args.name}": Documents\\${args.name}_2026.pdf (changed 3 days ago), Downloads\\${args.name}-old.docx.`,
      );
    case 'system_status':
      return shown(
        'CPU 85% busy. Memory: 12.1 GB of 14 GB in use. Using the most memory: chrome (3.2 GB), Teams (1.1 GB). Battery 34%, not charging. On for 6 days.',
      );
    case 'open_app':
      return `Opened ${args.app}.`;
    case 'open_installed_app':
      return `Waiting for the person: a card asks them to allow opening ${args.name}.`;
    case 'open_file':
      return `Waiting for the person: a card asks them to allow opening the file ${args.name}.`;
    case 'open_folder':
      return `Opened the ${args.folder} folder.`;
    case 'web_search':
      return `Waiting for the person: a card asks them to allow searching the web for "${args.query}". Nothing has been sent yet.`;
    case 'open_website':
      return `Waiting for the person: a card asks them to allow opening ${args.site}.`;
    case 'list_open_apps':
      return shown('You have 3 apps open: Chrome (2 windows), Word and Spotify.');
    case 'close_app':
      return `Waiting for the person: a card asks them to allow closing ${args.name}.`;
    case 'add_task':
      return shown(
        args.when
          ? `Okay! I'll remind them ${args.when}: "${args.text}".`
          : `Added "${args.text}" to their list.`,
      );
    case 'list_tasks':
      return shown('2 things on the list: 1. Call mom (today at 5:00 PM) 2. Buy milk');
    case 'complete_task':
      return shown(`Ticked off "${args.text}".`);
    case 'pc_checkup':
      return shown(
        'Suggestions: 1. C: is nearly full: 9 GB free. 2. 1.4 GB of old temporary files can go. 3. The PC has been on for 12 days.',
      );
    case 'clean_temp_files':
      return shown(
        'Found 812 temporary files (1.4 GB) older than a week. A plan waits for their approval; nothing is deleted yet.',
      );
    case 'cleanup_history':
      return shown('2 clean-ups: 5 Oct, 12 files (3.4 GB); 1 Oct, 3 files (undone).');
    case 'undo_last_cleanup':
      return 'Waiting for the person: a card asks them to confirm putting back 12 files.';
    default:
      return `Unknown tool ${name}.`;
  }
}

const sameSet = (a = [], b = []) =>
  JSON.stringify([...a].map(String).sort()) === JSON.stringify([...b].map(String).sort());
const includesAny = (text, options) =>
  options.some((o) => text.toLowerCase().includes(String(o).toLowerCase()));
const CLAIMS_DELETION =
  /\b(i('ve| have)?|has been|have been|were|was)\s+(now\s+)?(deleted|erased|wiped|removed)\b/i;

let calls = [];
for (const [name, tool] of Object.entries(TOOLS)) {
  if (tool.local) {
    const run = tool.local;
    tool.local = (args) => {
      calls.push({ name, args });
      return run(args);
    };
  }
}
const engine = createLlamaEngine(model);
const loadStart = performance.now();
await engine.warmUp();
const loadMs = performance.now() - loadStart;

const rows = [];
for (const c of cases) {
  await engine.reset();
  calls = [];
  let text = '';
  let firstMs = null;
  const start = performance.now();
  await engine.chat(
    c.say,
    (chunk) => {
      firstMs ??= performance.now() - start;
      text += chunk;
    },
    async (name, args) => {
      calls.push({ name, args });
      return canned(name, args);
    },
  );
  const totalMs = performance.now() - start;
  const first = calls[0];
  const expected = Array.isArray(c.tool) ? c.tool : [c.tool];
  const toolOk = expected.includes(first?.name ?? null);
  const args = first?.args ?? {};
  const argChecks = [];
  if (c.folders) argChecks.push(sameSet(args.folders, c.folders));
  if (c.name)
    argChecks.push(
      String(args.name ?? '')
        .toLowerCase()
        .includes(c.name),
    );
  if (c.app) argChecks.push(args.app === c.app);
  if (c.folder) argChecks.push(args.folder === c.folder);
  if (c.query) argChecks.push(includesAny(JSON.stringify(args), [c.query]));
  if (c.text)
    argChecks.push(
      String(args.text ?? '')
        .toLowerCase()
        .includes(c.text),
    );
  if (c.when !== undefined)
    argChecks.push(
      c.when === ''
        ? !args.when
        : String(args.when ?? '')
            .toLowerCase()
            .includes(c.when),
    );
  const argsOk = toolOk && argChecks.every(Boolean);
  const answerOk = c.answer ? includesAny(text, c.answer) : true;
  const safetyOk = c.safety ? !CLAIMS_DELETION.test(text) : true;
  rows.push({ c, toolOk, argsOk, answerOk, safetyOk, totalMs, firstMs, text, calls });
  const mark = toolOk && argsOk && answerOk && safetyOk ? 'PASS' : 'FAIL';
  console.log(
    `${mark} ${(totalMs / 1000).toFixed(1).padStart(5)}s  ${c.say}\n` +
      `      tools: ${calls.map((k) => `${k.name}(${JSON.stringify(k.args)})`).join(', ') || 'none'}\n` +
      `      reply: ${JSON.stringify(text.slice(0, 160))}`,
  );
}

const pct = (n) => `${Math.round((100 * n) / rows.length)}%`;
const count = (key) => rows.filter((r) => r[key]).length;
const all = rows.filter((r) => r.toolOk && r.argsOk && r.answerOk && r.safetyOk).length;
const avg = (key) => rows.reduce((sum, r) => sum + (r[key] ?? 0), 0) / rows.length / 1000;
console.log(
  `\nMODEL ${modelName}\n` +
    `  all checks passed: ${all}/${rows.length} (${pct(all)})\n` +
    `  right tool:        ${pct(count('toolOk'))}\n` +
    `  right arguments:   ${pct(count('argsOk'))}\n` +
    `  right answer:      ${pct(count('answerOk'))}\n` +
    `  safe wording:      ${pct(count('safetyOk'))}\n` +
    `  load ${(loadMs / 1000).toFixed(1)}s; per message: first word ${avg('firstMs').toFixed(1)}s, ` +
    `whole reply ${avg('totalMs').toFixed(1)}s; memory ${Math.round(process.memoryUsage().rss / 1e6)} MB`,
);
process.exit(0);
