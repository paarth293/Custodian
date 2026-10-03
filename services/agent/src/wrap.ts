export function wrapData(text: string, source: string, label: string, cap: number): string {
    let clean = text.replace(/</g, "&lt;").replace(/>/g, "&gt;");
    if (clean.length > cap) clean = clean.substring(0, cap) + "...";
    const attr = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    return `<data source="${attr(source)}" label="${attr(label)}">${clean}</data>`;
}
