import { z } from 'zod';

export const REQUEST_COOLDOWN_MS = 10 * 60 * 1000;

export const RequestAccessSchema = z.object({
  level: z.string(),
  reason_code: z.enum([
    'needs_web_search',
    'needs_download',
    'needs_install',
    'needs_online_voice',
    'needs_helper_sync',
  ]),
});

export type AccessResult = { status: 'Requested'; error?: undefined } | { error: string };

/** Validates and rate-limits the agent's requests for elevated access. */
export class AccessManager {
  private lastRequest: number | undefined;

  constructor(private readonly now: () => number = Date.now) {}

  requestAccess(args: unknown): AccessResult {
    if (!RequestAccessSchema.safeParse(args).success) {
      return { error: 'Rejected: invalid reason_code' };
    }
    const now = this.now();
    if (this.lastRequest !== undefined && now - this.lastRequest < REQUEST_COOLDOWN_MS) {
      return { error: 'Rate limit exceeded' };
    }
    this.lastRequest = now;
    return { status: 'Requested' };
  }
}
