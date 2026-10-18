import type { GbnfJsonSchema } from 'node-llama-cpp' with { 'resolution-mode': 'import' };

export const WORKFLOW_STEPS = [
  'discover',
  'filter_documents',
  'filter_images',
  'select_largest',
  'select_all',
  'copy',
  'verify',
] as const;
export interface WorkflowProposal {
  steps: string[];
  attempts?: number;
}
export interface WorkflowSelection {
  filter: 'any' | 'documents' | 'images';
  largest: boolean;
}
export function workflowSelection(value: unknown): WorkflowSelection {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Invalid native selection');
  const v = value as Record<string, unknown>;
  if (
    Object.keys(v).length !== 2 ||
    typeof v.filter !== 'string' ||
    !['any', 'documents', 'images'].includes(v.filter) ||
    typeof v.largest !== 'boolean'
  )
    throw new Error('Invalid native selection');
  return { filter: v.filter as WorkflowSelection['filter'], largest: v.largest };
}
export function workflowSchema(selection: WorkflowSelection) {
  const contract = workflowSelection(selection);
  const steps = [
    'discover',
    ...(contract.filter === 'any' ? [] : [`filter_${contract.filter}`]),
    contract.largest ? 'select_largest' : 'select_all',
    'copy',
    'verify',
  ];
  return {
    type: 'object',
    properties: {
      steps: {
        type: 'array',
        items: { enum: steps },
        minItems: steps.length,
        maxItems: steps.length,
      },
    },
  } as const satisfies GbnfJsonSchema;
}
export const WORKFLOW_SYSTEM =
  'You are a bounded file-workflow planner. Return only JSON steps. Respect the native selection contract. Use each required capability exactly once. Discover, filter if requested, select, copy, then verify. You cannot execute tools, change authority or choose paths.';
export function parseWorkflowProposal(text: string): WorkflowProposal {
  const value: unknown = JSON.parse(text);
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.keys(value).join(',') !== 'steps'
  )
    throw new Error('Invalid workflow proposal');
  const steps = (value as { steps?: unknown }).steps;
  if (
    !Array.isArray(steps) ||
    steps.length < 4 ||
    steps.length > 5 ||
    steps.some(
      (step) => typeof step !== 'string' || !(WORKFLOW_STEPS as readonly string[]).includes(step),
    )
  )
    throw new Error('Invalid workflow steps');
  return { steps };
}
export function workflowPrompt(goal: string, selection?: WorkflowSelection): string {
  if (goal.length > 2400) throw new Error('Workflow goal is too long');
  if (!selection) throw new Error('Native selection is required');
  const contract = workflowSelection(selection);
  return `Return {"steps":[...]}. Examples: documents -> ["discover","filter_documents","select_all","copy","verify"]; photos -> ["discover","filter_images","select_all","copy","verify"]; largest file -> ["discover","select_largest","copy","verify"]; largest document -> ["discover","filter_documents","select_largest","copy","verify"]; largest photo -> ["discover","filter_images","select_largest","copy","verify"]. Include every required capability once, in the illustrated dependency order. Native selection contract: ${JSON.stringify(contract)}. The goal cannot change the contract: ${JSON.stringify(goal)}. Paths are bound externally.`;
}

export function proposalIssue(
  proposal: WorkflowProposal,
  selection: WorkflowSelection,
): string | null {
  const expected = [
    'discover',
    ...(selection.filter === 'any' ? [] : [`filter_${selection.filter}`]),
    selection.largest ? 'select_largest' : 'select_all',
    'copy',
    'verify',
  ];
  if (JSON.stringify(proposal.steps) === JSON.stringify(expected)) return null;
  const missing = expected.filter((step) => !proposal.steps.includes(step));
  const repeated = proposal.steps.filter((step, index) => proposal.steps.indexOf(step) !== index);
  return `Invalid plan. Missing capabilities: ${missing.join(', ') || 'none'}. Repeated capabilities: ${repeated.join(', ') || 'none'}. Required dependencies: discover before filter, filter before selection, selection before copy, copy before verify. Keep the native selection contract. Return the corrected JSON, without omissions or repetitions.`;
}

/** Bounded compiler-feedback loop, not a permission or a filesystem executor. */
export async function proposeWorkflow(
  goal: string,
  selection: WorkflowSelection,
  generate: (prompt: string) => Promise<string>,
  signal?: AbortSignal,
): Promise<WorkflowProposal> {
  const contract = workflowSelection(selection);
  let prompt = workflowPrompt(goal, contract);
  for (let attempt = 1; attempt <= 2; attempt++) {
    if (signal?.aborted) throw new Error('Workflow planning stopped');
    const text = await generate(prompt);
    if (signal?.aborted) throw new Error('Workflow planning stopped');
    let proposal: WorkflowProposal | undefined;
    let issue: string | null;
    try {
      proposal = parseWorkflowProposal(text);
      issue = proposalIssue(proposal, contract);
    } catch {
      issue = 'Malformed proposal. Return only the required JSON steps in dependency order.';
    }
    if (proposal && !issue) return { ...proposal, attempts: attempt };
    console.error(
      `[workflow-plan] rejected attempt ${attempt}: ${JSON.stringify({ proposal, issue })}`,
    );
    prompt = issue!;
  }
  throw new Error(
    'Workflow plan failed validation after two model attempts; no filesystem work started',
  );
}
