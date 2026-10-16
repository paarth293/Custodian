import { TOOLS } from './tools';

/** Desktop history is data, not part of the current request's tool routing. */
export function requestText(message: string): string {
  const start = '<data>\nRecent PC activity and verified results:\n';
  const end = '\n</data>\n\n';
  if (message.startsWith(start)) {
    const boundary = message.indexOf(end, start.length);
    if (boundary >= 0) return message.slice(boundary + end.length);
  }
  return message;
}

/** These destructive capabilities are not implemented; never solicit a fake approval. */
export function unsupportedDestructiveRequest(message: string): boolean {
  return /^(?:(?:please|can you|could you|would you|i want you to)\s+)*(?:wipe|erase|permanently erase|delete all)\b|^ignore\b.*\b(?:wipe|erase)\b/i.test(
    requestText(message).trim(),
  );
}

/** Select descriptions for this request, not authority to execute anything.
 * Every selected call still passes through the desktop's argument and approval checks.
 * Keeping unrelated schemas out also avoids evaluating thousands of tokens on a CPU.
 */
export function selectToolNames(message: string): string[] {
  const text = requestText(message).toLowerCase();
  if (/^(?:write|compose|translate|tell me a joke)\b/.test(text.trim())) return [];
  const asksDefinition = /(?:^|:\s*)(?:what (?:is|are)|define|explain)\b/.test(text);
  if (
    asksDefinition &&
    !/\b(?:my|current|today|now|this pc|here|time|date|weekday|day of the week)\b|\b[a-z]\s*(?:drive|:)/.test(
      text,
    )
  )
    return [];
  if (/\b(?:text editor|simple editor)\b/.test(text)) return ['open_app'];
  if (/\b(?:what|which|show|list)\b.*\bsettings\b.*\b(?:can|support)\b/.test(text))
    return ['list_settings'];
  if (/\b(?:make|set|turn)\b.*\b(?:windows|apps?)\b.*\bdark\b/.test(text))
    return ['change_setting'];
  if (/\b(?:spotify|vlc)\b/.test(text) && !/\b(?:close|quit|shut)\b/.test(text))
    return ['open_installed_app'];
  if (
    /\b(?:close|quit|shut)\b/.test(text) &&
    /\b(?:spotify|vlc|notepad|calculator|paint|chrome|word|excel|teams)\b/.test(text)
  )
    return ['close_app'];
  // These capabilities have distinct meanings even when they contain file/app words.
  if (
    /\b(?:display settings|screen resolution|display scaling|sound settings|network settings)\b/.test(
      text,
    )
  )
    return ['open_app'];
  if (
    /\b(?:dark mode|transparency|taskbar|file (?:name )?extensions|hidden(?: system)? files)\b/.test(
      text,
    )
  ) {
    if (
      /\b(?:make|turn|set|move|stop showing|show|hide|enable|disable)\b/.test(text) &&
      !/\b(?:what|which|list)\b/.test(text)
    )
      return ['change_setting'];
    return ['list_settings', 'change_setting', 'open_app'];
  }
  if (
    /\b(?:bring up|open|show)\s+(?:my |the )?(?:pictures|downloads|documents|desktop|videos|music)\s+folder\b/.test(
      text,
    )
  )
    return ['open_folder'];
  if (/^i\s+(?:(?:have|already|just)\s+)*(?:bought|finished|completed)\b/.test(text.trim()))
    return ['complete_task'];
  const changesFiles = /\b(?:move|copy|rename|create)\b/.test(text);
  if (
    !changesFiles &&
    /\b(?:clutter|cluttered|checkup|check-up|suggestions|where should i start)\b/.test(text)
  )
    return ['pc_checkup'];
  if (!changesFiles && /\b(?:largest|biggest|big|huge|large|hogging|chunkiest)\b/.test(text))
    return ['find_large_files'];
  if (!changesFiles && /\b(?:duplicate|duplicates|copies|identical|twice)\b/.test(text))
    return ['find_duplicate_files'];
  const names = new Set<string>();
  const add = (...tools: string[]) => tools.forEach((name) => names.add(name));

  if (/\b(?:space|disk|drive|storage|room|full)\b/.test(text)) add('check_disk_space');
  if (/\b(?:largest|biggest|big|huge|large|hogging|chunkiest)\b/.test(text))
    add('find_large_files');
  if (/\b(?:duplicate|duplicates|copies|identical|twice)\b/.test(text)) add('find_duplicate_files');
  if (
    /\b(?:file|files|document|documents|resume|spreadsheet|spreadsheets|photos?|pictures?|videos?|saved|folder|folders)\b/.test(
      text,
    )
  ) {
    if (/\b(?:details|properties|inspect|info|how big|size of)\b/.test(text)) add('get_file_info');
    else add('find_files');
  }
  if (
    /\b(?:cpu|memory|ram|battery|slow|sluggish|crawling|performance|fan|uptime|switched on|running for)\b/.test(
      text,
    )
  )
    add('system_status');
  if (
    /\b(?:suggest|suggestions|advice|clutter|cluttered|tidy|checkup|check-up|where should i start|what should i do)\b/.test(
      text,
    )
  )
    add('pc_checkup');
  if (/\b(?:temp|temporary|junk)\b/.test(text)) add('clean_temp_files');
  if (/\b(?:cleaned|history|so far|what did you do)\b/.test(text)) add('cleanup_history');
  if (/\b(?:undo|restore|put back)\b/.test(text)) add('undo_last_cleanup');
  if (/\b(?:privacy|private|safe|security|protect|protection)\b/.test(text)) add('privacy_report');

  if (
    /\b(?:open|launch|start|fire up|editor|notepad|calculator|paint|spotify|vlc|settings|explorer)\b/.test(
      text,
    )
  ) {
    add('open_app', 'open_installed_app', 'open_file', 'open_folder');
  }
  if (/\b(?:close|quit|shut|stop playing)\b/.test(text)) add('close_app');
  if (
    !names.has('system_status') &&
    /\b(?:apps|programs|windows|what's open|what is open)\b/.test(text)
  )
    add('list_open_apps');
  if (
    /\b(?:settings?|theme|dark|light mode|transparency|taskbar|extensions|hidden files|display)\b/.test(
      text,
    )
  ) {
    add('list_settings', 'change_setting', 'open_app');
  }
  if (
    /\b(?:remind|reminder|reminders|forget|to-do|todo|list|jot|note|buy|task|tasks|plate)\b/.test(
      text,
    )
  ) {
    add('add_task', 'list_tasks', 'complete_task');
  }
  if (/\b(?:already|finished|completed|done|bought)\b/.test(text))
    add('list_tasks', 'complete_task');
  if (/\b(?:weather|forecast|search|look online|look up|flights|news|internet|web)\b/.test(text))
    add('web_search');
  if (
    /\b(?:website|youtube|google|amazon|myntra|site)\b|\b[a-z0-9-]+\.(?:com|org|net|in)\b/.test(
      text,
    )
  )
    add('open_website', 'web_search');
  if (/\b(?:time|date|weekday|day of the week|what day|today)\b/.test(text))
    add('current_date_time');
  if (
    /\b(?:calculate|percent|times|minus|plus|divided|multiply|arithmetic|power)\b|\d\s*[+*/%^=]|\b\d+[.,]?\d*\s+(?:a|per)\s+month\b/.test(
      text,
    )
  )
    add('calculate');
  if (/\bmove\b/.test(text)) add('move_file', 'find_files', 'get_file_info');
  if (/\bcopy\b/.test(text)) add('copy_file', 'find_files', 'get_file_info');
  if (/\brename\b/.test(text)) add('rename_file', 'find_files', 'get_file_info');
  if (/\b(?:create|make|new)\s+(?:a\s+)?folder\b/.test(text)) add('create_folder');

  return [...names].filter((name) => Object.hasOwn(TOOLS, name));
}

/** Operational requests may need one bounded correction if the model only talks. */
export function requiresToolUse(message: string): boolean {
  const text = requestText(message).trim().toLowerCase();
  if (
    /^(?:what (?:is|are|does)|explain|define|write|compose|tell me about)\b/.test(text) &&
    !/\b(?:my|current|today|now|this pc|the time|day of the week)\b/.test(text)
  )
    return false;
  if (/\b(?:do not|don't|never|without asking)\b/.test(text)) return false;
  return selectToolNames(message).length > 0;
}
