import type { Plan } from '../components/ApprovalView';
import type { Message } from '../components/ChatPanel';

/** Events the assistant streams while answering (mirrors `assistant::ChatEvent`). */
export type ChatEvent =
  | { kind: 'text'; text: string }
  | { kind: 'status'; text: string }
  | { kind: 'plan'; plan: Plan }
  | { kind: 'action'; action: string };

/** Appends streamed text to the reply with `replyId`, creating it on first use. */
export function appendReply(messages: Message[], replyId: string, text: string): Message[] {
  if (!messages.some((m) => m.id === replyId)) {
    return [...messages, { id: replyId, role: 'assistant', content: text }];
  }
  return messages.map((m) => (m.id === replyId ? { ...m, content: m.content + text } : m));
}
