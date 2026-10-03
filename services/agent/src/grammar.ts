export function validateWithGrammar(output: string, _schema: any): any {
    try {
        return JSON.parse(output);
    } catch {
        throw new Error("E_SCHEMA");
    }
}
