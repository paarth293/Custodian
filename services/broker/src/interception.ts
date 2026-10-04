export type NetworkPolicy = 'any_https' | 'allow_all';

/** Applies the network policy to an outgoing request URL. */
export function interceptRequest(url: string, policy: NetworkPolicy): 'allow' | 'block' {
  if (policy === 'any_https' && !url.startsWith('https://')) return 'block';
  return 'allow';
}
