import { StrictMode, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import ApprovalView, { Plan } from './components/ApprovalView';

const root = document.getElementById('root');
if (!root) throw new Error('missing #root element');

const params = new URLSearchParams(window.location.search);

function render(node: ReactNode) {
  createRoot(root as HTMLElement).render(<StrictMode>{node}</StrictMode>);
}

// The approval window loads index.html?approve=<plan hash>.
const planHash = params.get('approve');

if (import.meta.env.DEV && params.has('gallery')) {
  // Development-only gallery of every Cubby mood: http://localhost:1420/?gallery
  void Promise.all([import('./gallery/Gallery'), import('./gallery/gallery.css')]).then(
    ([{ default: Gallery }]) => render(<Gallery />),
  );
} else if (planHash) {
  // In standalone approval window, we mock the plan fetch until IPC read API is fully wired
  const standalonePlan: Plan = {
    id: planHash,
    riskLevel: 'high',
    operations: [{ path: 'C:\\Target\\unknown.exe', action: 'quarantine', sizeBytes: 1024 }],
  };
  render(
    <ApprovalView
      plan={standalonePlan}
      onDecided={(approved, result) => console.log('Standalone decision:', approved, result)}
    />,
  );
} else {
  render(<App />);
}
