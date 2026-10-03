export function searchSerpApi(query: string, apiKey: string) {
    if (!apiKey) throw new Error("SerpApi key missing!");
    return [{ domain: "official.com", snippet: "official site" }];
}
