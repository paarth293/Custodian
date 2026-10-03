export function checkDlp(payload: string, bloomFilter: string[]) {
    for (const token of bloomFilter) {
        if (payload.includes(token)) return 'blocked';
    }
    return 'clean';
}
