import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import ApprovalView, { Plan } from './components/ApprovalView';

const root = document.getElementById('root');
if (!root) throw new Error('missing #root element');

// The approval window loads index.html?approve=<plan hash>.
const planHash = new URLSearchParams(window.location.search).get('approve');

// In standalone approval window, we mock the plan fetch until IPC read API is fully wired
const standalonePlan: Plan | null = planHash ? {
  id: planHash,
  riskLevel: 'high',
  operations: [{ path: 'C:\\Target\\unknown.exe', action: 'quarantine', sizeBytes: 1024 }]
} : null;

createRoot(root).render(
  <StrictMode>
    {standalonePlan ? (
      <ApprovalView 
        plan={standalonePlan} 
        onDecided={(approved) => console.log('Standalone decision:', approved)} 
      />
    ) : (
      <App />
    )}
  </StrictMode>
);
