import { useState } from 'react';
import './App.css';
import Cubby from './components/Cubby';
import ChatPanel, { Message } from './components/ChatPanel';

function App() {
  const [expanded, setExpanded] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isThinking, setIsThinking] = useState(false);

  const handleSendMessage = (text: string) => {
    // Optimistically add user message
    const userMsg: Message = { id: Date.now().toString(), role: 'user', content: text };
    setMessages((prev) => [...prev, userMsg]);
    setIsThinking(true);
    
    // Placeholder for actual LLM call (Step 4 & 5)
    setTimeout(() => {
      const replyMsg: Message = { 
        id: (Date.now() + 1).toString(), 
        role: 'assistant', 
        content: `I received your message: "${text}". The AI brain will be wired up soon!` 
      };
      setMessages((prev) => [...prev, replyMsg]);
      setIsThinking(false);
    }, 1000);
  };

  return (
    <div className={`app-container${expanded ? ' expanded' : ''}`}>
      <div className="canvas-panel" data-tauri-drag-region>
        <button
          type="button"
          className="cubby-wrapper"
          aria-expanded={expanded}
          aria-label={expanded ? 'Hide chat' : 'Show chat'}
          onClick={() => setExpanded((value) => !value)}
        >
          <Cubby />
        </button>
      </div>

      {expanded && (
        <ChatPanel 
          messages={messages} 
          isThinking={isThinking} 
          onSendMessage={handleSendMessage} 
        />
      )}
    </div>
  );
}

export default App;
