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
interface AppSettings {
  revision: number;
  privateExecutables: string[];
}

export default function PrivacyControls({ onChanged }: { onChanged: () => void }) {
  const [open, setOpen] = useState(false);
  const [settings, setSettings] = useState<Settings>();
  const [paths, setPaths] = useState<string[]>([]);
  const [path, setPath] = useState('');
  const [capabilities, setCapabilities] = useState<Capability[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [appSettings, setAppSettings] = useState<AppSettings>();
  const [executables, setExecutables] = useState<string[]>([]);
  const [executable, setExecutable] = useState('');
  useEffect(() => {
    if (!open) return;
    let active = true;
    void Promise.all([
      call<Settings>('get_privacy_settings'),
      call<Capability[]>('get_capabilities'),
      call<AppSettings>('get_app_privacy_settings'),
    ])
      .then(([current, available, apps]) => {
        if (!active) return;
        if (!current) {
          setMessage('Privacy controls require the desktop application.');
          return;
        }
        setSettings(current);
        setPaths(current.privatePaths);
        setCapabilities(available ?? []);
        setAppSettings(apps ?? undefined);
        setExecutables(apps?.privateExecutables ?? []);
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
  const saveApps = async () => {
    if (!appSettings || busy) return;
    setBusy(true);
    setMessage('Saving application exclusions and stopping existing tasks…');
    try {
      const saved = await call<AppSettings>('set_private_applications', {
        executables,
        revision: appSettings.revision,
      });
      if (!saved) throw new Error('The desktop application is unavailable.');
      setAppSettings(saved);
      setExecutables(saved.privateExecutables);
      onChanged();
      setMessage(
        'Application exclusions saved. Tasks were asked to stop and the conversation cleared.',
      );
    } catch {
      setAppSettings(undefined);
      setMessage(
        'Application rules could not be saved. Close and reopen these controls to reload them. If access is paused, restart the app.',
      );
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
          if (!open) {
            setSettings(undefined);
            setAppSettings(undefined);
            setMessage('Loading privacy rules…');
          }
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
          <h3>Never access these applications</h3>
          <p>
            Add the full executable path without opening or inspecting the application. Exclusions
            persist across restarts. Qualified tasks access only their own bound document. Screen
            capture and general app reading remain disabled. These exclusions do not block the
            application itself or isolate its network access. Use private locations above to exclude
            its files from file tools too.
          </p>
          <ul>
            {executables.map((item) => (
              <li key={item}>
                <span>{item}</span>
                <button
                  type="button"
                  disabled={busy || !appSettings}
                  aria-label={`Remove private application ${item}`}
                  onClick={() => setExecutables(executables.filter((entry) => entry !== item))}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
          <label>
            Full application executable path
            <input
              aria-label="Private application executable path"
              value={executable}
              disabled={busy || !appSettings}
              onChange={(event) => setExecutable(event.target.value)}
            />
          </label>
          <button
            type="button"
            disabled={busy || !appSettings || !executable.trim()}
            onClick={() => {
              const next = executable.trim();
              if (!executables.includes(next)) setExecutables([...executables, next]);
              setExecutable('');
            }}
          >
            Add application
          </button>
          <button type="button" disabled={busy || !appSettings} onClick={() => void saveApps()}>
            Save application exclusions
          </button>
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
