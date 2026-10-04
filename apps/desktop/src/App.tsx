import { useState } from 'react';
import { invoke, Channel } from '@tauri-apps/api/core';
import './App.css';
import Cubby from './components/Cubby';
import ChatPanel, { Message } from './components/ChatPanel';

function App() {
  const [expanded, setExpanded] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isThinking, setIsThinking] = useState(false);

  const handleSendMessage = async (text: string) => {
    // Optimistically add user message
    const userMsg: Message = { id: Date.now().toString(), role: 'user', content: text };
    setMessages((prev) => [...prev, userMsg]);
    setIsThinking(true);
    
    // Create assistant message stub
    const replyId = (Date.now() + 1).toString();
    const replyMsg: Message = { id: replyId, role: 'assistant', content: '' };
    setMessages((prev) => [...prev, replyMsg]);
    setIsThinking(false);
    
    // Create IPC channel to receive streaming tokens
    const channel = new Channel<string>();
    channel.onmessage = (token: string) => {
      setMessages((prev) => prev.map(msg => 
        msg.id === replyId ? { ...msg, content: msg.content + token } : msg
      ));
    };
    
    try {
      await invoke('send_chat_message', { message: text, channel });
    } catch (err) {
      console.error('Chat error:', err);
      setMessages((prev) => prev.map(msg => 
        msg.id === replyId ? { ...msg, content: msg.content + "\n[Connection error]" } : msg
      ));
    }
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
