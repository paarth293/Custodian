import { z } from 'zod';

export const RequestAccessSchema = z.object({
    level: z.string(),
    reason_code: z.enum(['needs_web_search', 'needs_download', 'needs_install', 'needs_online_voice', 'needs_helper_sync'])
});

export class AccessManager {
    lastRequest: number = 0;

    requestAccess(args: any) {
        const parsed = RequestAccessSchema.safeParse(args);
        if (!parsed.success) return { error: "Rejected: invalid reason_code" };
        
        const now = Date.now();
        if (now - this.lastRequest < 10 * 60 * 1000) return { error: "Rate limit exceeded" };
        
        this.lastRequest = now;
        return { status: "Requested" };
    }
}
