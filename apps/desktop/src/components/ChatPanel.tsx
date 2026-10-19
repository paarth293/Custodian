import { useEffect, useRef, useState } from 'react';
import type { ActionCard } from '../chat/events';
import { call } from '../lib/tauri';
import TaskBoard from './TaskBoard';

export interface ActiveTask {
  id: string;
  description: string;
  state: string;
  progress?: string;
  application?: { document_id: string; workspace?: string } | null;
  execution?: {
    program: {
      goal: string;
      constraints: string[];
      steps: { title: string; kind: string; capability?: string }[];
    };
    outputs: Record<string, { receipts: string[] }>;
    skipped: number[];
    in_flight: number | null;
    pause_requested: boolean;
    paused: boolean;
    verified: boolean;
    failure: { class: string; detail: string } | null;
  };
}

export type Message = {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  /** A tool's result, written by Cubby's own code rather than the AI. */
  result?: boolean;
  /** Something the AI asked for that waits for the person's click. */
  card?: ActionCard;
  /** The person's answer to `card`. */
  decided?: 'allowed' | 'declined';
};

export interface ChatPanelProps {
  disabled?: boolean;
  messages: Message[];
  /** True while Cubby is answering; the input waits. */
  isThinking: boolean;
  /** Live progress line, e.g. "Scanning… 12,000 files". */
  status?: string | null;
  onSendMessage: (text: string) => void;
  /** Stops the answer or scan in progress. */
  onStop?: () => void;
  /** The person's answer to the card in message `messageId`. */
  onDecideCard?: (messageId: string, allow: boolean) => void;
}

const SUGGESTIONS = [
  'Find duplicate files',
  'Biggest files in Downloads',
  'How much free space?',
  'Is my PC slow?',
  'What can you do?',
];

function CardView({
  message,
  onDecide,
}: {
  message: Message & { card: ActionCard };
  onDecide?: (messageId: string, allow: boolean) => void;
}) {
  const { card, decided } = message;
  return (
    <div className="chat-message assistant">
      <span className="chat-role">Cubby asks</span>
      <section className="action-card" aria-label={card.title}>
        <h3>{card.title}</h3>
        <p>{card.detail}</p>
        {decided ? (
          <p className="action-card-decided">{decided === 'allowed' ? 'Allowed' : 'Not now'}</p>
        ) : (
          <div className="approval-actions">
            <button
              type="button"
              className="btn-reject"
              onClick={() => onDecide?.(message.id, false)}
            >
              Not now
            </button>
            <button
              type="button"
              className="btn-approve"
              onClick={() => onDecide?.(message.id, true)}
            >
              {card.confirm}
            </button>
          </div>
        )}
      </section>
    </div>
  );
}

export default function ChatPanel({
  disabled = false,
  messages,
  isThinking,
  status,
  onSendMessage,
  onStop,
  onDecideCard,
}: ChatPanelProps) {
  const [input, setInput] = useState('');
  const [activeTasks, setActiveTasks] = useState<ActiveTask[]>([]);
  const endOfMessagesRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Poll active tasks periodically
  useEffect(() => {
    let mounted = true;
    const fetchTasks = async () => {
      try {
        const list = await call<ActiveTask[]>('list_active_tasks');
        if (mounted && list) {
          setActiveTasks(list);
        }
      } catch {
        // Ignored in non-Tauri or testing environment
      }
    };
    void fetchTasks();
    const interval = setInterval(fetchTasks, 1000);
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  const handleCancelTask = async (taskId: string) => {
    try {
      await call('cancel_task', { taskId });
      const list = await call<ActiveTask[]>('list_active_tasks');
      if (list) {
        setActiveTasks(list);
      }
    } catch {
      // Ignored
    }
  };

  const handlePauseTask = async (taskId: string, paused: boolean) => {
    try {
      await call('pause_task', { taskId, paused });
      const list = await call<ActiveTask[]>('list_active_tasks');
      if (list) setActiveTasks(list);
    } catch (error) {
      console.error('Could not change task pause state:', error);
    }
  };

  // Auto-scroll to bottom when messages change
  useEffect(() => {
    endOfMessagesRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking, status]);

  useEffect(() => {
    if (!isThinking) inputRef.current?.focus();
  }, [isThinking]);

  const send = (text: string) => {
    if (!disabled && text.trim()) {
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
          messages.map((msg) =>
            msg.card ? (
              <CardView
                key={msg.id}
                message={msg as Message & { card: ActionCard }}
                onDecide={onDecideCard}
              />
            ) : (
              <div
                key={msg.id}
                className={`chat-message ${msg.role}${msg.result ? ' result' : ''}`}
              >
                <span className="chat-role">{msg.role === 'user' ? 'You' : 'Cubby'}</span>
                <p className="chat-content">{msg.content}</p>
              </div>
            ),
          )
        )}
        {isThinking && (
          <div className="chat-message assistant thinking" aria-live="polite">
            <span className="chat-role">Cubby</span>
            <p className="chat-content">{status ?? 'Thinking…'}</p>
          </div>
        )}
        <div ref={endOfMessagesRef} />
      </div>

      <TaskBoard
        activeTasks={activeTasks}
        onCancelTask={handleCancelTask}
        onPauseTask={handlePauseTask}
        onInspectWorkflow={() => onSendMessage('show workflow status')}
        onInspectApplication={(id) => onSendMessage(`inspect application task ${id}`)}
      />

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
          aria-label="Chat input"
          maxLength={2000}
          disabled={disabled}
        />
        {isThinking && (
          <button
            type="button"
            className="chat-stop"
            onClick={() => onStop?.()}
            aria-label="Stop current work"
          >
            Stop
          </button>
        )}
        <button type="submit" disabled={disabled || !input.trim()}>
          Send
        </button>
      </form>
    </section>
  );
}
