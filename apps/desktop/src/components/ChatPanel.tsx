import { useEffect, useRef, useState } from 'react';

export type Message = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
};

export interface ChatPanelProps {
  messages: Message[];
  isThinking: boolean;
  onSendMessage: (text: string) => void;
}

export default function ChatPanel({ messages, isThinking, onSendMessage }: ChatPanelProps) {
  const [input, setInput] = useState('');
  const endOfMessagesRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when messages change
  useEffect(() => {
    endOfMessagesRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (input.trim() && !isThinking) {
      onSendMessage(input.trim());
      setInput('');
    }
  };

  return (
    <section className="chat-panel" aria-label="Conversation">
      <div className="chat-history">
        {messages.length === 0 ? (
          <p className="chat-empty">How can I help you today?</p>
        ) : (
          messages.map((msg) => (
            <div key={msg.id} className={`chat-message ${msg.role}`}>
              <span className="chat-role">{msg.role === 'user' ? 'You' : 'Cubby'}</span>
              <p className="chat-content">{msg.content}</p>
            </div>
          ))
        )}
        {isThinking && (
          <div className="chat-message assistant thinking">
            <span className="chat-role">Cubby</span>
            <p className="chat-content">Thinking...</p>
          </div>
        )}
        <div ref={endOfMessagesRef} />
      </div>

      <form className="chat-input-form" onSubmit={handleSubmit}>
        <input
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
