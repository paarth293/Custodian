import { useState } from 'react';
import { invoke, Channel } from '@tauri-apps/api/core';
import './App.css';
import Cubby from './components/Cubby';
import ChatPanel, { Message } from './components/ChatPanel';
import ApprovalView, { Plan } from './components/ApprovalView';

function App() {
  const [expanded, setExpanded] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isThinking, setIsThinking] = useState(false);
  
  // Phase 4: Holds the plan awaiting user approval
  const [pendingPlan, setPendingPlan] = useState<Plan | null>(null);

  const handleSendMessage = async (text: string) => {
    const userMsg: Message = { id: Date.now().toString(), role: 'user', content: text };
    setMessages((prev) => [...prev, userMsg]);
    
    // Developer mock: typing "plan" triggers a test plan
    if (text.toLowerCase() === 'plan') {
      setPendingPlan({
        id: 'plan-12345',
        riskLevel: 'high',
        operations: [
          { path: 'C:\\Users\\Downloads\\sus_file.exe', action: 'quarantine', sizeBytes: 154000 },
          { path: 'C:\\Users\\Downloads\\malware.dll', action: 'delete', sizeBytes: 88000 }
        ]
      });
      return;
    }

    setIsThinking(true);
    
    const replyId = (Date.now() + 1).toString();
    const replyMsg: Message = { id: replyId, role: 'assistant', content: '' };
    setMessages((prev) => [...prev, replyMsg]);
    setIsThinking(false);
    
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

  const handleApprovalDecided = (approved: boolean) => {
    setMessages((prev) => [...prev, {
      id: Date.now().toString(),
      role: 'assistant',
      content: approved ? 'Plan approved and executed successfully.' : 'Plan was rejected.'
    }]);
    setPendingPlan(null);
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
        <div className="side-panel">
          <ChatPanel 
            messages={messages} 
            isThinking={isThinking} 
            onSendMessage={handleSendMessage} 
          />
          {pendingPlan && (
            <ApprovalView 
              plan={pendingPlan} 
              onDecided={handleApprovalDecided} 
            />
          )}
        </div>
      )}
    </div>
  );
}

export default App;
