import type { Plan } from '../components/ApprovalView';
import type { Message } from '../components/ChatPanel';

/** Something the AI asked for that waits for the person's click (mirrors `actions::ActionCard`). */
export type ActionCard = {
  id: string;
  title: string;
  detail: string;
  /** Label of the button that allows it. */
  confirm: string;
};

/** Events the assistant streams while answering (mirrors `assistant::ChatEvent`). */
export type ChatEvent =
  | { kind: 'text'; text: string }
  | { kind: 'status'; text: string }
  | { kind: 'plan'; plan: Plan }
  | { kind: 'action'; action: string }
  /** What a tool found, written by Cubby's code rather than the AI. */
  | { kind: 'result'; text: string }
  | { kind: 'confirm'; card: ActionCard };

/** Appends streamed text to the reply with `replyId`, creating it on first use. */
export function appendReply(messages: Message[], replyId: string, text: string): Message[] {
  if (!messages.some((m) => m.id === replyId)) {
    return [...messages, { id: replyId, role: 'assistant', content: text }];
  }
  return messages.map((m) => (m.id === replyId ? { ...m, content: m.content + text } : m));
}

/** Longest speech bubble: about two short lines (one idea per bubble, §11). */
const BUBBLE_CHARS = 70;

/** The first sentence of `text`, short enough for Cubby's speech bubble. */
export function bubbleText(text: string): string {
  const first = text.split(/(?<=[.!?])\s/)[0] ?? text;
  return first.length > BUBBLE_CHARS ? `${first.slice(0, BUBBLE_CHARS - 1).trimEnd()}…` : first;
}

/** Records the person's answer to the card in message `id`. */
export function decideCard(
  messages: Message[],
  id: string,
  decided: 'allowed' | 'declined',
): Message[] {
  return messages.map((m) => (m.id === id && m.card && !m.decided ? { ...m, decided } : m));
}
