export function interceptRequest(url: string, policy: string) {
    if (url.trim().toLowerCase().startsWith('http://') && policy === 'any_https') return 'block';
    return 'allow';
}
