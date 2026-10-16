import type { Plan } from '../components/ApprovalView';

export type ConversationWork = { turns: Record<string, string | null>; plans: Plan[] };
export type WorkEvent =
  | { kind: 'start'; id: string }
  | { kind: 'status'; id: string; text: string }
  | { kind: 'finish'; id: string }
  | { kind: 'plan'; id: string; plan: Plan }
  | { kind: 'decided'; planId: string }
  | { kind: 'reset' };

export function updateWork(state: ConversationWork, event: WorkEvent): ConversationWork {
  if (event.kind === 'reset') return { turns: {}, plans: [] };
  if (event.kind === 'start') return { ...state, turns: { ...state.turns, [event.id]: null } };
  if (event.kind === 'decided')
    return { ...state, plans: state.plans.filter((p) => p.id !== event.planId) };
  if (!Object.hasOwn(state.turns, event.id)) return state;
  if (event.kind === 'status')
    return { ...state, turns: { ...state.turns, [event.id]: event.text } };
  if (event.kind === 'plan') {
    return { ...state, plans: [...state.plans.filter((p) => p.id !== event.plan.id), event.plan] };
  }
  const turns = { ...state.turns };
  delete turns[event.id];
  return { ...state, turns };
}
