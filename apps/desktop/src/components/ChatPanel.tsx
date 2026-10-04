import { useEffect, useRef, useState } from 'react';

export type Message = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
};

export interface ChatPanelProps {
  messages: Message[];
  /** True while Cubby is answering; the input waits. */
  isThinking: boolean;
  /** Live progress line, e.g. "Scanning… 12,000 files". */
  status?: string | null;
  onSendMessage: (text: string) => void;
}

const SUGGESTIONS = [
  'Find duplicate files',
  'Biggest files in Downloads',
  'How much free space?',
  'What can you do?',
];

export default function ChatPanel({ messages, isThinking, status, onSendMessage }: ChatPanelProps) {
  const [input, setInput] = useState('');
  const endOfMessagesRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-scroll to bottom when messages change
  useEffect(() => {
    endOfMessagesRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking, status]);

  useEffect(() => {
    if (!isThinking) inputRef.current?.focus();
  }, [isThinking]);

  const send = (text: string) => {
    if (text.trim() && !isThinking) {
      onSendMessage(text.trim());
      setInput('');
    }
  };

  return (
    <section className="chat-panel" aria-label="Conversation">
      <div className="chat-history">
        {messages.length === 0 ? (
          <div className="chat-empty">
            <p>How can I help you today?</p>
            <div className="chat-suggestions">
              {SUGGESTIONS.map((s) => (
                <button key={s} type="button" onClick={() => send(s)}>
                  {s}
                </button>
              ))}
            </div>
          </div>
        ) : (
          messages.map((msg) => (
            <div key={msg.id} className={`chat-message ${msg.role}`}>
              <span className="chat-role">{msg.role === 'user' ? 'You' : 'Cubby'}</span>
              <p className="chat-content">{msg.content}</p>
            </div>
          ))
        )}
        {isThinking && (
          <div className="chat-message assistant thinking" aria-live="polite">
            <span className="chat-role">Cubby</span>
            <p className="chat-content">{status ?? 'Thinking…'}</p>
          </div>
        )}
        <div ref={endOfMessagesRef} />
      </div>

      <form
        className="chat-input-form"
        onSubmit={(e) => {
          e.preventDefault();
          send(input);
        }}
      >
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Ask Cubby something..."
          disabled={isThinking}
          aria-label="Chat input"
        />
        <button type="submit" disabled={!input.trim() || isThinking}>
          Send
        </button>
      </form>
    </section>
  );
}
