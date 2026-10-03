import { z } from 'zod';

export const tools = {
    quarantine: {
        name: 'quarantine',
        schema: z.object({ target_id: z.string() }),
        execute: async (args: any) => { return { status: 'mocked_ipc', args }; }
    }
};
