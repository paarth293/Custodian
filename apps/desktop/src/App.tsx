import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import './App.css';
import Cubby from './components/Cubby';

function App() {
  const [expanded, setExpanded] = useState(false);
  const [ledger, setLedger] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!expanded) return;
    invoke<string[]>('get_ledger')
      .then((entries) => {
        setLedger(entries);
        setError(null);
      })
      .catch((err: unknown) => setError(String(err)));
  }, [expanded]);

  return (
    <div className={`app-container${expanded ? ' expanded' : ''}`}>
      <div className="canvas-panel" data-tauri-drag-region>
        <button
          type="button"
          className="cubby-wrapper"
          aria-expanded={expanded}
          aria-label={expanded ? 'Hide ledger' : 'Show ledger'}
          onClick={() => setExpanded((value) => !value)}
        >
          <Cubby />
        </button>
      </div>

      {expanded && (
        <section className="settings-panel" aria-label="Ledger and settings">
          <h2>Ledger &amp; Settings</h2>
          {error ? (
            <p role="alert">Could not load the ledger: {error}</p>
          ) : (
            <ul>
              {ledger.map((entry, index) => (
                <li key={index}>{entry}</li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}

export default App;
