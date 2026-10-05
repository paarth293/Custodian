import type { GbnfJsonSchema } from 'node-llama-cpp' with { 'resolution-mode': 'import' };
import { calculate } from './calculator';

/**
 * Cubby's closed tool vocabulary (§40, §88): the only things the language
 * model can ask for. Every parameter is an enum or a short search text, so
 * nothing the model writes can become a path, a command or a URL. The desktop
 * app validates each call again and decides what really happens; anything
 * that changes files or sends data off the PC only proposes, and the person
 * decides on screen.
 */

export const FOLDERS = [
  'Downloads',
  'Documents',
  'Desktop',
  'Pictures',
  'Videos',
  'Music',
] as const;

export const APPS = [
  'notepad',
  'calculator',
  'paint',
  'file_explorer',
  'settings',
  'task_manager',
] as const;

/** Runs a desktop tool and returns its result as text for the model. */
export type ToolRunner = (name: string, args: Record<string, unknown>) => Promise<string>;

export interface ToolDefinition {
  description: string;
  /** JSON schema for the arguments, enforced by a grammar while the model writes them. */
  params?: GbnfJsonSchema;
  /** Pure tools that run inside the agent, with no access to the PC. */
  local?: (args: Record<string, unknown>) => string;
}

const folders = {
  type: 'array',
  description: 'Folders to look in. An empty list means all of them.',
  items: { enum: FOLDERS },
  maxItems: FOLDERS.length,
} as const;

const shortText = (description: string) =>
  ({ type: 'string', description, minLength: 1, maxLength: 100 }) as const;

/** Local date and time, e.g. "Monday, 5 October 2026, 9:41 AM". */
export function currentDateTime(now = new Date()): string {
  const date = now.toLocaleDateString('en-GB', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });
  const time = now.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
  return `${date}, ${time} (the PC's local time)`;
}

export const TOOLS: Record<string, ToolDefinition> = {
  check_disk_space: {
    description: 'Free and total space on each of the PC’s drives.',
  },
  find_large_files: {
    description: 'Lists the biggest files in the person’s folders.',
    params: { type: 'object', properties: { folders } },
  },
  find_duplicate_files: {
    description:
      'Finds identical copies of files and proposes moving the extra copies to Cubby’s safe place. ' +
      'Nothing moves unless the person approves it.',
    params: { type: 'object', properties: { folders } },
  },
  find_files: {
    description:
      'Finds files whose names contain the given words, for example "resume" or "tax 2025". ' +
      'Use it when the person asks where a file is.',
    params: {
      type: 'object',
      properties: { name: shortText('Words from the file name.'), folders },
    },
  },
  system_status: {
    description:
      'Current CPU and memory use, the apps using the most memory, battery level and uptime. ' +
      'Use it when the PC seems slow or the person asks about battery or performance.',
  },
  open_app: {
    description: 'Opens one of these Windows apps.',
    params: { type: 'object', properties: { app: { enum: APPS } } },
  },
  open_installed_app: {
    description:
      'Opens any app installed on the PC by its name, for example "Spotify" or "VLC". The person must allow it.',
    params: {
      type: 'object',
      properties: { name: shortText('The app name, as in the Start menu.') },
    },
  },
  open_file: {
    description:
      'Opens one of the person’s documents, photos, songs or videos by words from its name, with its usual app. ' +
      'Never opens programs. The person must allow it.',
    params: {
      type: 'object',
      properties: {
        name: shortText('Words from the file name, for example "resume" or "tax 2025".'),
      },
    },
  },
  open_folder: {
    description: 'Opens one of the person’s folders in File Explorer.',
    params: { type: 'object', properties: { folder: { enum: FOLDERS } } },
  },
  web_search: {
    description:
      'Prepares a web search in the browser. The person sees the exact search and must allow it before anything is sent.',
    params: { type: 'object', properties: { query: shortText('What to search for.') } },
  },
  open_website: {
    description:
      'Prepares opening a website, for example "youtube" or "example.com". The person must allow it.',
    params: {
      type: 'object',
      properties: { site: shortText('A site name or domain, without any path.') },
    },
  },
  list_open_apps: {
    description: 'Lists the apps open on the desktop right now, such as Chrome or Word.',
  },
  close_app: {
    description:
      'Closes an open app by its name, like clicking its X, so it can ask to save work first. ' +
      'The person must allow it.',
    params: {
      type: 'object',
      properties: { name: shortText('The app name, for example "Notepad" or "Chrome".') },
    },
  },
  add_task: {
    description:
      'Adds something to the person’s to-do list, optionally with a reminder. ' +
      'Put the time in "when" in words, exactly as the person said it, for example ' +
      '"at 5pm", "in 20 minutes" or "tomorrow at 9am"; use "" for no reminder. Never work out dates yourself.',
    params: {
      type: 'object',
      properties: {
        text: shortText('What to do, in the person’s own words, without the time.'),
        when: {
          type: 'string',
          description: 'When to remind them, in words, or "" for no reminder.',
          maxLength: 100,
        },
      },
    },
  },
  list_tasks: {
    description: 'Shows the person’s to-do list and reminders.',
  },
  complete_task: {
    description: 'Ticks off an item on the to-do list, by words from it or its number on the list.',
    params: {
      type: 'object',
      properties: { text: shortText('Words from the item, or its number.') },
    },
  },
  privacy_report: {
    description:
      'How Cubby protects the person’s data and privacy on this PC, checked right now. ' +
      'Use it for any question about safety, privacy, uploads or who can see their files.',
  },
  pc_checkup: {
    description:
      'A quick check-up with suggestions: nearly full drives, old temporary files, the Recycle Bin, ' +
      'memory, restarts and reminders due today. Use it when the person asks what to do or how to free space.',
  },
  clean_temp_files: {
    description:
      'Finds temporary files apps left behind more than a week ago and proposes deleting them. ' +
      'Nothing is deleted unless the person approves.',
  },
  cleanup_history: {
    description: 'Lists Cubby’s recent clean-ups.',
  },
  undo_last_cleanup: {
    description:
      'Offers to put back the files from the most recent clean-up. The person confirms it.',
  },
  calculate: {
    description:
      'Exact arithmetic. Use it for any calculation instead of working it out yourself. ' +
      'Write percentages like "17% * 2350".',
    params: {
      type: 'object',
      properties: { expression: shortText('For example "(1250 + 380) * 12".') },
    },
    local: (args) => calculate(String(args['expression'] ?? '')),
  },
  current_date_time: {
    description: 'Today’s date and the current time.',
    local: () => currentDateTime(),
  },
};
