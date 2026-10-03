import { z } from 'zod';

export const tools = {
    quarantine: {
        name: 'quarantine',
        schema: z.object({ target_id: z.string() }),
        execute: async (args: unknown) => { return { status: 'mocked_ipc', args }; }
    }
};
