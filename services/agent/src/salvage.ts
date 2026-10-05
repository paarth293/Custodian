/**
 * Small local models sometimes write a tool call out as text
 * (`<tool_call>{"name": "check_disk_space", "arguments": {}}</tool_call>`)
 * instead of making it. Shown as is, that would be raw JSON in the chat and
 * nothing would happen. These helpers recognise a reply that is only such a
 * call, so the agent can make the call properly: it then goes through the
 * same checks in the desktop app as any other tool call.
 */

/** A tool call recovered from text. */
export interface WrittenCall {
  name: string;
  args: Record<string, unknown>;
}

/** Tags small models wrap tool calls in. */
const WRAPPERS = /<\/?(tool_call|tools|tool|function_call|functions?)>/gi;

/**
 * Whether a reply starting with `lead` may be a written-out tool call, so its
 * text should be held back until the reply is complete.
 */
export function mightBeToolCall(lead: string): boolean {
  const text = lead.trimStart();
  return text.startsWith('<') || text.startsWith('{') || text.startsWith('```');
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * The tool call that `text` consists of, if it names one of `known`: one
 * JSON object with `name` and `arguments` (or `function` and `arguments`),
 * optionally inside tags or a code fence. Anything else is `null`.
 */
export function parseWrittenCall(text: string, known: ReadonlySet<string>): WrittenCall | null {
  const body = text
    .replace(/^\s*```(?:json)?/i, '')
    .replace(/```\s*$/, '')
    .replace(WRAPPERS, '')
    .trim();
  if (!body.startsWith('{') || !body.endsWith('}')) return null;
  let value: unknown;
  try {
    value = JSON.parse(body);
  } catch {
    return null;
  }
  if (!isObject(value)) return null;
  const fn = value['function'];
  const name =
    typeof value['name'] === 'string'
      ? value['name']
      : typeof fn === 'string'
        ? fn
        : isObject(fn) && typeof fn['name'] === 'string'
          ? fn['name']
          : null;
  if (name === null || !known.has(name)) return null;
  const raw =
    value['arguments'] ?? value['parameters'] ?? (isObject(fn) ? fn['arguments'] : undefined);
  let args: unknown = raw ?? {};
  if (typeof args === 'string') {
    try {
      args = JSON.parse(args);
    } catch {
      return null;
    }
  }
  return isObject(args) ? { name, args } : null;
}
