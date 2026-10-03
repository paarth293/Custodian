import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';
import Cubby from './components/Cubby';

function App() {
  const [expanded, setExpanded] = useState(false);
  const [ledger, setLedger] = useState<string[]>([]);

  useEffect(() => {
    if (expanded) {
      invoke<string[]>('get_ledger').then(setLedger).catch(console.error);
    }
  }, [expanded]);

  return (
    <div className={`app-container ${expanded ? 'expanded' : ''}`}>
      <div className="canvas-panel" data-tauri-drag-region>
        <div className="cubby-wrapper" onClick={() => setExpanded(!expanded)}>
          <Cubby />
        </div>
      </div>
      
      {expanded && (
        <div className="settings-panel">
          <h2>Ledger & Settings</h2>
          <ul>
            {ledger.map((item, idx) => (
              <li key={idx}>{item}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export default App;
