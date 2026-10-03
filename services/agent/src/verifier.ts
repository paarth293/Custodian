export function verifyExplanation(draft: string, evidenceDates: string[], template: string): string {
    const dates = draft.match(/\b\d{4}-\d{2}-\d{2}\b/g) || [];
    for (const date of dates) {
        if (!evidenceDates.includes(date)) return template;
    }
    return draft;
}
