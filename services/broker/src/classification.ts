export function classifyAction(actionType: string, _url: string, _payload: unknown) {
    if (['payment', 'login', 'message'].includes(actionType)) return 'handover';
    return 'proceed';
}
