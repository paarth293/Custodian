const HANDOVER_ACTIONS = new Set(['payment', 'login', 'message']);

/** Sensitive actions are handed back to the user instead of automated. */
export function classifyAction(actionType: string): 'handover' | 'proceed' {
  return HANDOVER_ACTIONS.has(actionType) ? 'handover' : 'proceed';
}
