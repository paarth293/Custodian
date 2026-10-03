export const BROWSER_ACTIONS = [
  'navigate',
  'search',
  'read',
  'click',
  'type',
  'select',
  'scroll',
  'download',
  'submit',
  'handover',
] as const;

export type BrowserAction = (typeof BROWSER_ACTIONS)[number];

export function isBrowserAction(action: string): action is BrowserAction {
  return (BROWSER_ACTIONS as readonly string[]).includes(action);
}

/**
 * Drives an isolated Chromium profile on the agent's behalf.
 * Placeholder: actions are validated but not executed yet.
 */
export class BrowserBroker {
  readonly profile = 'isolated_profile';

  /** Refuses to launch with Chromium's sandbox disabled. */
  start(flags: string[]): void {
    if (flags.includes('--no-sandbox'))
      throw new Error('E_POLICY: Chromium sandbox must stay enabled');
  }

  async performAction(action: string, params: unknown): Promise<string> {
    void params;
    if (!isBrowserAction(action)) throw new Error('E_POLICY');
    return `Action ${action} executed`;
  }
}
