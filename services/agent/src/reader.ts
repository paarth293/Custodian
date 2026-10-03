const INJECTION_MARKERS = ['ignore your instructions', 'ignore previous instructions'];

export interface ReaderResult {
  data: { summary: string };
  label: 'WEB_UNTRUSTED';
  injectionDetected: boolean;
}

/**
 * Extracts data from untrusted web chunks. Output is always labelled
 * untrusted; chunks containing prompt-injection markers are flagged.
 * Placeholder: the summary is fixed until extraction is wired in.
 */
export function readerExtract(chunks: string[]): ReaderResult {
  const injectionDetected = chunks.some((chunk) =>
    INJECTION_MARKERS.some((marker) => chunk.toLowerCase().includes(marker)),
  );
  if (injectionDetected) console.log('sec.event: injection attempt detected');
  return { data: { summary: 'extracted' }, label: 'WEB_UNTRUSTED', injectionDetected };
}
