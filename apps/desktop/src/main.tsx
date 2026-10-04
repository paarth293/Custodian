import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import ApprovalView from './components/ApprovalView';

const root = document.getElementById('root');
if (!root) throw new Error('missing #root element');

// The approval window loads index.html?approve=<plan hash>.
const planHash = new URLSearchParams(window.location.search).get('approve');

createRoot(root).render(
  <StrictMode>{planHash ? <ApprovalView planHash={planHash} /> : <App />}</StrictMode>,
);
