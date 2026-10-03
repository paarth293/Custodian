export function validateWithGrammar(output: string, _schema: unknown): unknown {
    try {
        return JSON.parse(output);
    } catch {
        throw new Error("E_SCHEMA");
    }
}
