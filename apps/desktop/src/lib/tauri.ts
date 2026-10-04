import { invoke, isTauri } from '@tauri-apps/api/core';

/** True inside the desktop app; false in a plain browser (dev gallery, tests). */
export const inTauri = isTauri();

/** Invokes a Tauri command, or resolves to `undefined` outside the app. */
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T | undefined> {
  if (!inTauri) return undefined;
  return invoke<T>(command, args);
}
