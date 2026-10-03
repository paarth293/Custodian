import { z } from 'zod';

/** Tools the agent may call. Execution is forwarded to the engine over IPC. */
export const tools = {
  quarantine: {
    name: 'quarantine',
    schema: z.object({ target_id: z.string() }),
    execute: async (args: unknown) => ({ status: 'mocked_ipc', args }),
  },
};
