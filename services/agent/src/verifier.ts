const ISO_DATE = /\b\d{4}-\d{2}-\d{2}\b/g;

/**
 * Returns `draft` only if every ISO date it mentions appears in the evidence;
 * otherwise falls back to the safe `template`.
 */
export function verifyExplanation(
  draft: string,
  evidenceDates: string[],
  template: string,
): string {
  const dates = draft.match(ISO_DATE) ?? [];
  return dates.every((date) => evidenceDates.includes(date)) ? draft : template;
}
