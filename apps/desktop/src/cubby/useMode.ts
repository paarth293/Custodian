import { listen } from '@tauri-apps/api/event';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import { call, inTauri } from '../lib/tauri';
import type { Form } from './types';

export interface Mode {
  /** Real Online mode (broker running). */
  online: boolean;
  /** Special Form preview from the tray or Cubby's menu. */
  demo: boolean;
}

export const MODE_EVENT = 'mode-changed';

export function formFor(mode: Mode): Form {
  return mode.online || mode.demo ? 'special' : 'home';
}

const OFFLINE: Mode = { online: false, demo: false };

/**
 * Current mode from the desktop shell, kept live via the mode-changed event.
 * `onChange(next, previous)` runs whenever the shell reports a change.
 */
export function useMode(onChange?: (next: Mode, previous: Mode) => void): Mode {
  const [mode, setMode] = useState<Mode>(OFFLINE);
  const current = useRef<Mode>(OFFLINE);
  const changed = useEffectEvent((next: Mode, previous: Mode) => onChange?.(next, previous));

  useEffect(() => {
    if (!inTauri) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const apply = (next: Mode) => {
      const previous = current.current;
      current.current = next;
      setMode(next);
      changed(next, previous);
    };
    void call<Mode>('get_mode').then((m) => {
      if (m && !cancelled) apply(m);
    });
    void listen<Mode>(MODE_EVENT, (event) => apply(event.payload)).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return mode;
}
