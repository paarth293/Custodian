import type { GbnfJsonSchema } from 'node-llama-cpp' with { 'resolution-mode': 'import' };
import { TOOLS } from './tools';

/** A bounded retry must choose a typed call or an explicit clarification.
 * This constrains syntax only; the desktop still checks arguments and approval.
 */
export function toolPlanSchema(names: readonly string[]): GbnfJsonSchema {
  const selected = names.filter((name) => Object.hasOwn(TOOLS, name));
  // These calls have a default scope or are selected only after recognising
  // their target. Mutating/external calls still stop at the desktop approval card.
  // Requests with missing file-operation details retain a clarification option.
  const defaultSearches = new Set([
    'find_large_files',
    'find_duplicate_files',
    'find_files',
    'open_app',
    'open_folder',
    'open_installed_app',
    'web_search',
    'change_setting',
    'calculate',
    'add_task',
    'complete_task',
    'open_website',
    'close_app',
  ]);
  const needsDetails =
    selected.length === 0 ||
    selected.some((name) => !!TOOLS[name]!.params && !defaultSearches.has(name));
  return {
    oneOf: [
      ...selected.map((name) => ({
        type: 'object' as const,
        properties: {
          name: { const: name },
          arguments: TOOLS[name]!.params ?? { type: 'object' as const, properties: {} },
        },
      })),
      ...(needsDetails
        ? [
            {
              type: 'object',
              properties: {
                question: {
                  type: 'string',
                  minLength: 1,
                  maxLength: 240,
                  description: 'One question, only if a required detail is missing or ambiguous.',
                },
              },
            } as const,
          ]
        : []),
    ],
  };
}
