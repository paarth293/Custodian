export function wrapData(text: string, source: string, label: string, cap: number): string {
    let clean = text.replace(/</g, "&lt;").replace(/>/g, "&gt;");
    if (clean.length > cap) clean = clean.substring(0, cap) + "...";
    return <data source=" + source + " label=" + label + "> + clean + </data>;
}
