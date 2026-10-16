/**
 * A small model sometimes says it did something ("I've closed Spotify for
 * you", "I found your resume in Documents") without calling a tool, so
 * nothing happened. The person must never be left believing that, so the
 * agent adds a plain correction to such a reply.
 */

/** Saying an action was done or a result was seen, in the first person. */
const CLAIM =
  /\b(?:I(?:'ve|’ve| have)?|I just)\s+(?:now\s+|already\s+|successfully\s+)?(?:found|located|checked|looked|scanned|searched|opened|launched|started|closed|quit|deleted|removed|moved|cleaned|cleared|emptied|freed|installed|uninstalled|sent|added|set|scheduled|created|saved|renamed|restored|undone|put back)\b/i;

/** "Done!", "has been opened", "is now closed": the same, without "I". */
const PASSIVE =
  /\b(?:has|have)\s+been\s+(?:successfully\s+)?(?:opened|closed|deleted|removed|moved|cleaned|installed|sent|added|set|scheduled|created|saved|restored)\b|\bis\s+now\s+(?:open|closed|running|deleted|installed|set)\b/i;

/**
 * Promising to do it now ("I'll open Notepad for you right away"): without a
 * tool call nothing follows, and the person would wait for nothing.
 */
const PROMISE =
  /\bI(?:'ll|’ll| will)\s+(?:now\s+|quickly\s+|go ahead and\s+)?(?:open|launch|start|close|quit|delete|remove|move|clean|clear|empty|install|uninstall|send|add|set|schedule|create|save|rename|restore|undo|search|look up|find|check)\b/i;

/** Whether `text` claims an action or a result, or promises one. */
export function claimsAction(text: string): boolean {
  return CLAIM.test(text) || PASSIVE.test(text) || PROMISE.test(text);
}

/** Stating that an action mutated or altered PC state (open/close apps, delete/move files, change settings). */
const MUTATING_CLAIM =
  /\b(?:I(?:'ve|’ve| have)?|I just)\s+(?:now\s+|already\s+|successfully\s+)?(?:opened|launched|started|closed|quit|deleted|removed|moved|cleaned|cleared|emptied|freed|installed|uninstalled|sent|added|set|scheduled|created|saved|renamed|restored|undone|put back)\b/i;

const MUTATING_PASSIVE =
  /\b(?:has|have)\s+been\s+(?:successfully\s+)?(?:opened|closed|deleted|removed|moved|cleaned|installed|sent|added|set|scheduled|created|saved|restored)\b|\bis\s+now\s+(?:open|closed|running|deleted|installed|set)\b/i;

const MUTATING_PROMISE =
  /\bI(?:'ll|’ll| will)\s+(?:now\s+|quickly\s+|go ahead and\s+)?(?:open|launch|start|close|quit|delete|remove|move|clean|clear|empty|install|uninstall|send|add|set|schedule|create|save|rename|restore|undo)\b/i;

/** Whether `text` claims a mutating or state-changing action on the PC. */
export function claimsMutation(text: string): boolean {
  return MUTATING_CLAIM.test(text) || MUTATING_PASSIVE.test(text) || MUTATING_PROMISE.test(text);
}

/** Stating that files or items were deleted, erased, wiped or removed. */
export const CLAIMS_DELETION =
  /\b(?:I(?:'ve|’ve| have)?|has been|have been|were|was)\s+(?:now\s+)?(?:deleted|erased|wiped|removed)\b/i;

export function claimsDeletion(text: string): boolean {
  return CLAIMS_DELETION.test(text);
}

/** Added after a reply that claimed something no tool did. */
export const NOTHING_HAPPENED =
  '\n\n(To be clear: I haven’t actually done or checked that. Nothing on your PC was looked at or changed. Ask me again and I’ll use my tools.)';

/** Added when a model claims a deletion, since cleanup in Custodian always requires explicit approval card first. */
export const NOT_DELETED_YET =
  '\n\n(Note: Nothing has been deleted or removed yet. Any file deletion or cleanup requires your explicit approval on a confirmation card.)';

/** Only desktop-authored outcomes may describe a PC mutation as completed.
 * The tool result has already been rendered by the desktop; do not replace it
 * with an unverified model paraphrase, including one following a failed call.
 */
export function actionNarration(results: readonly string[]): string {
  if (
    results.some((result) => /waiting for (?:the person|your)|awaiting.*approval/i.test(result))
  ) {
    return 'The confirmation card is waiting for you. The proposed action has not run.';
  }
  return 'The action result is shown above.';
}
