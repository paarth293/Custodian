import type { GbnfJsonSchema } from 'node-llama-cpp' with { 'resolution-mode': 'import' };
import { WORKFLOW_STEPS, proposalIssue, type WorkflowSelection } from './workflow-plan';

export interface IntentPlan extends WorkflowSelection {
  decision: 'copy' | 'clarify' | 'unavailable';
  source: number;
  destination: number;
  nodes: { op: string; after: number }[];
  completion: 'all_copies_verified';
  attempts?: number;
}
/** These constraints have no registered executor. Never discard them to fit a plan. */
export function unsupportedIntent(message: string): boolean {
  const words = message
    .replace(/LOCATION_[01]/g, '')
    .toLowerCase()
    .split(/[^a-z0-9]+/);
  const blocked = new Set([
    'those',
    'except',
    'excluding',
    'exclude',
    'invoices',
    'containing',
    'contains',
    'password',
    'passwords',
    'newer',
    'older',
    'yesterday',
    'today',
    'tomorrow',
    'recent',
    'recently',
    'modified',
    'since',
    'before',
    'after',
    'unless',
    'if',
    'only',
    'smallest',
    'newest',
    'delete',
    'erase',
    'remove',
    'move',
    'rename',
    'wipe',
    'overwrite',
    'replace',
    'email',
    'upload',
    'download',
    'sendmail',
    'execute',
    'run',
    'powershell',
    'terminal',
    'script',
    'ignore',
  ]);
  const docs = words.some((w) => ['documents', 'document', 'docs', 'paperwork'].includes(w));
  const images = words.some((w) =>
    ['images', 'image', 'photos', 'photo', 'pictures', 'picture'].includes(w),
  );
  return (
    !words.some((w) =>
      [
        'copy',
        'copies',
        'copying',
        'backup',
        'back',
        'duplicate',
        'collect',
        'gather',
        'preserve',
      ].includes(w),
    ) ||
    Array.from(message).some((character) => character.charCodeAt(0) > 127) ||
    words.some((w) => blocked.has(w) || /^\d+$/.test(w)) ||
    (docs && images)
  );
}
export const INTENT_SYSTEM =
  'Classify an offline copy goal. Labels: documents (docs/paperwork), images (photos/pictures), files (everything/all files), largest_document (biggest document/paperwork), largest_image (biggest photo/picture/image), largest_file (biggest file). Biggest=largest. source is the LOCATION copied FROM; destination is copied TO. Examples: collect documents from LOCATION_0 to LOCATION_1 -> {"intent":"documents","source":0,"destination":1}; gather photos from LOCATION_0 into LOCATION_1 -> {"intent":"images","source":0,"destination":1}; copy the largest document from LOCATION_1 to LOCATION_0 -> {"intent":"largest_document","source":1,"destination":0}. Save a copy of the largest photo -> intent largest_image. Unsupported/uncertain goals: clarify. JSON only. No tools or authority.';
export const INTENT_SCHEMA = {
  type: 'object',
  properties: {
    intent: {
      enum: [
        'documents',
        'images',
        'files',
        'largest_document',
        'largest_image',
        'largest_file',
        'clarify',
        'unavailable',
      ],
    },
    source: { enum: [0, 1] },
    destination: { enum: [0, 1] },
  },
} as const satisfies GbnfJsonSchema;
export function parseIntentPlan(text: string): IntentPlan {
  const p = JSON.parse(text) as IntentPlan;
  if (
    !p ||
    typeof p !== 'object' ||
    Array.isArray(p) ||
    Object.keys(p).sort().join(',') !==
      'completion,decision,destination,filter,largest,nodes,source' ||
    !['copy', 'clarify', 'unavailable'].includes(p.decision) ||
    !['any', 'documents', 'images'].includes(p.filter) ||
    typeof p.largest !== 'boolean' ||
    ![0, 1].includes(p.source) ||
    ![0, 1].includes(p.destination) ||
    p.completion !== 'all_copies_verified' ||
    !Array.isArray(p.nodes) ||
    p.nodes.length > 5 ||
    p.nodes.some(
      (n) =>
        !n ||
        Object.keys(n).sort().join(',') !== 'after,op' ||
        !WORKFLOW_STEPS.includes(n.op as (typeof WORKFLOW_STEPS)[number]) ||
        !Number.isInteger(n.after),
    )
  )
    throw new Error('Invalid intent fields');
  if (
    p.decision === 'copy' &&
    (p.source === p.destination ||
      p.nodes.some((n, i) => n.after !== i - 1) ||
      proposalIssue({ steps: p.nodes.map((n) => n.op) }, p))
  )
    throw new Error('Invalid capabilities, dependencies or completion');
  return p;
}
export async function proposeIntent(
  message: string,
  generate: (prompt: string) => Promise<string>,
  signal?: AbortSignal,
): Promise<IntentPlan> {
  if (message.length > 2400) throw new Error('Intent request too long');
  if (unsupportedIntent(message))
    return {
      decision: 'clarify',
      filter: 'any',
      largest: false,
      source: 0,
      destination: 1,
      nodes: [],
      completion: 'all_copies_verified',
      attempts: 0,
    };
  const base = JSON.stringify({
    user_goal: message,
    native_observations: {
      LOCATION_0: 'permitted existing directory',
      LOCATION_1: 'permitted existing directory',
    },
    untrusted_content: [],
  });
  let feedback = '';
  for (let attempt = 1; attempt <= 2; attempt++) {
    if (signal?.aborted) throw new Error('Intent planning stopped');
    const text = await generate(base + feedback);
    if (signal?.aborted) throw new Error('Intent planning stopped');
    try {
      const choice = JSON.parse(text) as { intent: string; source: number; destination: number };
      if (
        !choice ||
        Object.keys(choice).sort().join(',') !== 'destination,intent,source' ||
        !INTENT_SCHEMA.properties.intent.enum.includes(
          choice.intent as (typeof INTENT_SCHEMA.properties.intent.enum)[number],
        ) ||
        ![0, 1].includes(choice.source) ||
        ![0, 1].includes(choice.destination) ||
        choice.source === choice.destination
      )
        throw Error('Invalid location selection');
      const decision =
        choice.intent === 'clarify'
          ? 'clarify'
          : choice.intent === 'unavailable'
            ? 'unavailable'
            : 'copy';
      const filter = choice.intent.includes('document')
        ? 'documents'
        : choice.intent.includes('image')
          ? 'images'
          : 'any';
      const largest = choice.intent.startsWith('largest_');
      const words = message.toLowerCase().split(/[^a-z]+/);
      const docs = words.some((w) => ['documents', 'document', 'docs', 'paperwork'].includes(w));
      const images = words.some((w) =>
        ['images', 'image', 'photos', 'photo', 'pictures', 'picture'].includes(w),
      );
      if (
        decision === 'copy' &&
        ((docs && filter !== 'documents') ||
          (images && filter !== 'images') ||
          (words.some((w) => ['largest', 'biggest'].includes(w)) && !largest))
      ) {
        feedback =
          '\nNative verifier: retain the explicit category (documents/docs/paperwork; images/photos/pictures) and largest/biggest constraint. Reinterpret the SAME goal.';
        continue;
      }
      const from = message.match(/from\s+LOCATION_([01])/i),
        to = message.match(/(?:to|into)\s+LOCATION_([01])/i);
      if (
        decision === 'copy' &&
        ((from && choice.source !== Number(from[1])) ||
          (to && choice.destination !== Number(to[1])))
      ) {
        feedback =
          '\nNative verifier: the source is the LOCATION after FROM; the destination is the LOCATION after TO/INTO. Keep the user direction.';
        continue;
      }
      const ops = [
        'discover',
        ...(filter === 'any' ? [] : [`filter_${filter}`]),
        largest ? 'select_largest' : 'select_all',
        'copy',
        'verify',
      ];
      return {
        decision,
        filter,
        largest,
        source: choice.source,
        destination: choice.destination,
        nodes: decision === 'copy' ? ops.map((op, i) => ({ op, after: i - 1 })) : [],
        completion: 'all_copies_verified',
        attempts: attempt,
      };
    } catch {
      feedback =
        '\nNative compiler feedback: invalid fields, dependency order or missing verification. Repair the JSON for the SAME user goal. Do not change intent to make a plan pass.';
    }
  }
  throw new Error(
    'Intent plan failed native-shape validation after two attempts; no effects started',
  );
}
