import type { ZodType } from 'zod';

/** Parses model output as JSON and validates it; throws `E_SCHEMA` otherwise. */
export function validateWithGrammar<T>(output: string, schema: ZodType<T>): T {
  let parsed: unknown;
  try {
    parsed = JSON.parse(output);
  } catch {
    throw new Error('E_SCHEMA');
  }
  const result = schema.safeParse(parsed);
  if (!result.success) throw new Error('E_SCHEMA');
  return result.data;
}
