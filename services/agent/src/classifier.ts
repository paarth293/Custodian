import { z } from 'zod';

export const ClassifierSchema = z.object({
    category: z.string(),
    label: z.string().max(50),
    looks_personal: z.boolean()
});

export function batchClassify(items: { id: string }[]) {
    return items.map(item => ({
        id: item.id,
        category: 'Document',
        label: 'User File',
        looks_personal: true
    }));
}
