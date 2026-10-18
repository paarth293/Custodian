import { useEffect, useRef, useState } from 'react';
import { call } from '../lib/tauri';

interface Settings {
  revision: number;
  automaticBackup: boolean;
  automaticCollections: boolean;
  sourceRoots: string[];
  destinationRoots: string[];
}

export default function RoutineControls({
  onChanged,
  onRequest,
}: {
  onChanged: () => void;
  onRequest: (text: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const [settings, setSettings] = useState<Settings>();
  const [enabled, setEnabled] = useState(false);
  const [collections, setCollections] = useState(false);
  const [sources, setSources] = useState('');
  const [destinations, setDestinations] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  useEffect(() => {
    if (!open) return;
    let active = true;
    void call<Settings>('get_routine_settings')
      .then((current) => {
        if (!active) return;
        if (!current) {
          setMessage('Routine permissions require the desktop app.');
          return;
        }
        setSettings(current);
        setEnabled(current.automaticBackup);
        setCollections(current.automaticCollections);
        setSources((current.sourceRoots ?? []).join('\n'));
        setDestinations((current.destinationRoots ?? []).join('\n'));
        setMessage('');
      })
      .catch(() => {
        if (active) setMessage('Routine permissions could not be loaded.');
      });
    return () => {
      active = false;
    };
  }, [open]);
  const save = async () => {
    if (!settings || busy) return;
    setBusy(true);
    setMessage('Saving. A copy already running must finish safely first…');
    try {
      const current = await call<Settings>('set_routine_settings', {
        revision: settings.revision,
        automaticBackup: enabled,
        automaticCollections: collections,
        sourceRoots: sources
          .split('\n')
          .map((path) => path.trim())
          .filter(Boolean),
        destinationRoots: destinations
          .split('\n')
          .map((path) => path.trim())
          .filter(Boolean),
      });
      if (!current) throw new Error('The desktop app is unavailable.');
      setSettings(current);
      setEnabled(current.automaticBackup);
      setCollections(current.automaticCollections);
      setSources((current.sourceRoots ?? []).join('\n'));
      setDestinations((current.destinationRoots ?? []).join('\n'));
      onChanged();
      setMessage(
        (current.automaticBackup
          ? 'Automatic backup enabled. Ask for a backup when you need one.'
          : 'Automatic backup disabled. Pending backup requests were stopped.') +
          (current.automaticCollections
            ? ' Verified collections enabled.'
            : ' Verified collections disabled.'),
      );
    } catch {
      setMessage('Permissions could not be saved. Reopen these controls and try again.');
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="privacy-controls">
      <button
        type="button"
        ref={trigger}
        aria-expanded={open}
        aria-controls={open ? 'routine-permissions-panel' : undefined}
        disabled={busy}
        onClick={() => {
          if (!open) {
            setSettings(undefined);
            setMessage('Loading routine permissions…');
          }
          setOpen((previous) => !previous);
        }}
      >
        Routine permissions
      </button>
      {open && (
        <section
          id="routine-permissions-panel"
          className="privacy-settings routine-settings"
          aria-label="Routine permissions controls"
        >
          <h3>Verified largest-file backup</h3>
          <p>
            These grants allow new verified copies only. They never allow moves, replacements,
            deletions, settings changes or control inside applications.
          </p>
          <label>
            Allowed source folders, one full path per line (optional)
            <textarea
              aria-label="Allowed routine source folders"
              value={sources}
              disabled={!settings || busy}
              onChange={(event) => setSources(event.target.value)}
            />
          </label>
          <label>
            Allowed destination folders, one full path per line (optional)
            <textarea
              aria-label="Allowed routine destination folders"
              value={destinations}
              disabled={!settings || busy}
              onChange={(event) => setDestinations(event.target.value)}
            />
          </label>
          <p>
            Leave a list empty to allow any permitted local location you explicitly request. Add
            folders to restrict that scope, including their subfolders. Both grants below use these
            limits. A request outside the allowed folders stops before discovery.
          </p>
          <label>
            <input
              type="checkbox"
              aria-label="Enable automatic verified backup"
              checked={enabled}
              disabled={!settings || busy}
              onChange={(event) => setEnabled(event.target.checked)}
            />
            Run this backup workflow automatically when I explicitly ask
          </label>
          <p>
            Search a requested local folder or all local drives, select the largest accessible
            permitted file, create a verified copy and save its undo record. No approval for each
            step.
          </p>
          <p>
            Both folders must exist. Up to 100 MiB per request, at most 5% of free space, leaving at
            least 1 GiB free. Private and protected locations remain excluded; existing files are
            never replaced.
          </p>
          <p>
            Example: Back up the largest file in "C:\Projects" to "D:\Backups". Use "all drives" as
            the source to search your PC.
          </p>
          <p>
            Other applications can react to new files. These controls do not isolate those apps from
            the network. Stop cancels discovery; a copy already running finishes safely.
          </p>
          <p>Saving permissions stops pending work and clears the conversation.</p>
          <h3>Model-planned verified collections</h3>
          <label>
            <input
              type="checkbox"
              aria-label="Enable automatic verified collections"
              checked={collections}
              disabled={!settings || busy}
              onChange={(event) => setCollections(event.target.checked)}
            />
            Allow Qwen to plan supported collections when I explicitly ask
          </label>
          <p>
            Native checks bind the selection, paths and authority. Up to 20 files and 100 MiB total
            per workflow, with the same free-space reserve. Originals stay unchanged and existing
            files are never replaced. Copy names are flattened; duplicate names stop preparation.
          </p>
          <p>
            Try: Collect documents from "C:\Projects" to "D:\Backups". Gather photos, or copy the
            largest document, using the same two quoted folders. Filters use filename extensions;
            document contents are not inspected for secrets.
          </p>
          <p>
            Stop ends the workflow between copies. A running copy finishes safely; completed copies
            remain available for group undo. Restart never automatically resumes a workflow.
          </p>
          <button type="button" disabled={!settings || busy} onClick={() => void save()}>
            Save routine permissions
          </button>
          <p role="status">{message}</p>
          <button
            type="button"
            disabled={busy}
            onClick={() => {
              setOpen(false);
              trigger.current?.focus();
              onRequest('show workflow status');
            }}
          >
            Show workflow status
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => {
              setOpen(false);
              trigger.current?.focus();
              onRequest('undo workflow');
            }}
          >
            Undo workflow
          </button>
        </section>
      )}
    </div>
  );
}
