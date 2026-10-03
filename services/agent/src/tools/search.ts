export interface WebResult {
  domain: string;
  snippet: string;
}

/** Online web search via SerpApi. Placeholder: returns a canned result. */
export function searchSerpApi(query: string, apiKey: string): WebResult[] {
  if (!apiKey) throw new Error('SerpApi key missing');
  void query;
  return [{ domain: 'official.com', snippet: 'official site' }];
}
