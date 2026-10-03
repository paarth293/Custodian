export function classifyAction(actionType: string, url: string, payload: any) {
    if (['payment', 'login', 'message'].includes(actionType)) return 'handover';
    return 'proceed';
}
