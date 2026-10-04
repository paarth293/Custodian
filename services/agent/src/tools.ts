import { z } from 'zod';
import { getLlama, LlamaJsonSchemaGrammar } from 'node-llama-cpp';

// Define the schemas for allowed tools
export const listCandidatesSchema = z.object({
  action: z.literal('list_candidates'),
  folder: z.string().describe('The folder to search in, e.g., "Downloads"'),
  older_than_days: z.number().optional(),
});

export const proposePlanSchema = z.object({
  action: z.literal('propose_plan'),
  candidate_ids: z.array(z.string()).describe('List of file IDs to quarantine/delete'),
  operation: z.enum(['quarantine', 'delete']),
});

export const answerSchema = z.object({
  action: z.literal('answer'),
  text: z.string().describe('The response to the user'),
});

// A union of all allowed actions
export const agentActionSchema = z.discriminatedUnion('action', [
  listCandidatesSchema,
  proposePlanSchema,
  answerSchema,
]);

export type AgentAction = z.infer<typeof agentActionSchema>;

/**
 * Creates a GBNF grammar for the Llama model to strictly enforce the agentActionSchema.
 * This guarantees the model will only ever output valid JSON matching our tools.
 */
export async function getActionGrammar() {
  const llama = await getLlama();
  
  // Convert Zod schema to a JSON schema compatible with node-llama-cpp
  const jsonSchema = {
    "type": "object",
    "properties": {
      "action": { "enum": ["list_candidates", "propose_plan", "answer"] },
      "folder": { "type": "string" },
      "older_than_days": { "type": "number" },
      "candidate_ids": { "type": "array", "items": { "type": "string" } },
      "operation": { "enum": ["quarantine", "delete"] },
      "text": { "type": "string" }
    },
    "required": ["action"]
  };
  
  return new LlamaJsonSchemaGrammar(llama, jsonSchema as any);
}
