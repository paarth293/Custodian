import { z } from 'zod';

export const ClassifierSchema = z.object({
  category: z.string(),
  label: z.string().max(50),
  looks_personal: z.boolean(),
});

export type Classification = z.infer<typeof ClassifierSchema> & { id: unknown };

/** Placeholder: labels every item as a personal document. */
export function batchClassify(items: Record<string, unknown>[]): Classification[] {
  return items.map((item) => ({
    id: item['id'],
    category: 'Document',
    label: 'User File',
    looks_personal: true,
  }));
}
