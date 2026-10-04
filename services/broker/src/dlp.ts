/** Blocks outgoing payloads that contain any protected token. */
export function checkDlp(payload: string, protectedTokens: readonly string[]): 'blocked' | 'clean' {
  return protectedTokens.some((token) => token !== '' && payload.includes(token))
    ? 'blocked'
    : 'clean';
}
