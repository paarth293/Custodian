import { useEffect, useState } from 'react';
import { call } from '../lib/tauri';

interface Settings {
  revision: number;
  privatePaths: string[];
  offline: boolean;
  notice: string;
}
interface Capability {
  name: string;
  status: string;
  detail: string;
}

export default function PrivacyControls({ onChanged }: { onChanged: () => void }) {
  const [open, setOpen] = useState(false);
  const [settings, setSettings] = useState<Settings>();
  const [paths, setPaths] = useState<string[]>([]);
  const [path, setPath] = useState('');
  const [capabilities, setCapabilities] = useState<Capability[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  useEffect(() => {
    if (!open) return;
    let active = true;
    void Promise.all([
      call<Settings>('get_privacy_settings'),
      call<Capability[]>('get_capabilities'),
    ])
      .then(([current, available]) => {
        if (!active) return;
        if (!current) {
          setMessage('Privacy controls require the desktop application.');
          return;
        }
        setSettings(current);
        setPaths(current.privatePaths);
        setCapabilities(available ?? []);
        setMessage('');
      })
      .catch(() => {
        if (active) setMessage('Privacy rules could not be loaded.');
      });
    return () => {
      active = false;
    };
  }, [open]);

  const save = async () => {
    if (!settings || busy) return;
    setBusy(true);
    setMessage('Saving rules and stopping existing tasks…');
    try {
      const saved = await call<Settings>('set_private_paths', {
        paths,
        revision: settings.revision,
      });
      if (!saved) throw new Error('The desktop application is unavailable.');
      setSettings(saved);
      setPaths(saved.privatePaths);
      onChanged();
      setMessage(
        'Saved. Previous tasks were asked to stop; approvals were invalidated and the conversation cleared.',
      );
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="privacy-controls">
      <button
        type="button"
        aria-expanded={open}
        disabled={busy}
        onClick={() => {
          if (!open) setMessage('Loading privacy rules…');
          setOpen(!open);
        }}
      >
        Privacy and capabilities
      </button>
      {open && (
        <section className="privacy-settings" aria-label="Privacy controls">
          <h3>Never access these locations</h3>
          <p>
            Choose existing files or folders on local drives. Other permitted locations remain
            searchable across your PC.
          </p>
          {settings && <p>{settings.notice}</p>}
          <ul>
            {paths.map((item) => (
              <li key={item}>
                <span>{item}</span>
                <button
                  type="button"
                  disabled={busy}
                  aria-label={`Remove private location ${item}`}
                  onClick={() => setPaths(paths.filter((entry) => entry !== item))}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
          <label>
            Absolute local path
            <input
              aria-label="Private location path"
              value={path}
              disabled={busy || !settings}
              onChange={(event) => setPath(event.target.value)}
            />
          </label>
          <button
            type="button"
            disabled={busy || !settings || !path.trim()}
            onClick={() => {
              const next = path.trim();
              if (!paths.includes(next)) setPaths([...paths, next]);
              setPath('');
            }}
          >
            Add location
          </button>
          <p>
            Saving requests current tasks to stop, invalidates approvals and clears the
            conversation. A file change already executing must finish safely before the rules take
            effect.
          </p>
          <button type="button" disabled={busy || !settings} onClick={() => void save()}>
            Save privacy rules
          </button>
          <p role="status">{message}</p>
          <h3>Available in this offline release</h3>
          <ul>
            {capabilities.map((capability) => (
              <li key={capability.name}>
                <strong>
                  {capability.name} — {capability.status}
                </strong>
                <p>{capability.detail}</p>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
