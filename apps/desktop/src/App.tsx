import { useState } from 'react';
import './App.css';
import Cubby from './components/Cubby';

function App() {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className={"app-container ${expanded ? 'expanded' : ''}"}>
      <div className="canvas-panel" data-tauri-drag-region>
        <div className="cubby-wrapper" onClick={() => setExpanded(!expanded)}>
          <Cubby />
        </div>
      </div>
      
      {expanded && (
        <div className="settings-panel">
          <h2>Ledger & Settings</h2>
          <p>Transaction history will appear here.</p>
        </div>
      )}
    </div>
  );
}

export default App;
