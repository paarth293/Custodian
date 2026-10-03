export function interceptRequest(url: string, policy: string) {
    if (url.startsWith('http://') && policy === 'any_https') return 'block';
    return 'allow';
}
