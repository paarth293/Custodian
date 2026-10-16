import { describe, expect, it } from 'vitest';
import { requiresToolUse, selectToolNames } from '../src/tool-selection';
import { TOOLS } from '../src/tools';

describe('request tool selection', () => {
  it('keeps general knowledge and prose generation free of PC tools', () => {
    expect(selectToolNames('What is a duplicate file?')).toEqual([]);
    expect(selectToolNames('Reply with one short sentence: what is a duplicate file?')).toEqual([]);
    expect(selectToolNames('Who wrote Pride and Prejudice?')).toEqual([]);
    expect(selectToolNames('Write a haiku about autumn rain')).toEqual([]);
    expect(selectToolNames('Hello!')).toEqual([]);
  });

  it.each([
    ['Find the biggest videos on my drive', 'find_large_files'],
    ['I need a text editor', 'open_app'],
    ['Change my display settings', 'open_app'],
    ['I already bought the batteries', 'complete_task'],
    ['Check the weather in Delhi', 'web_search'],
    ['What day of the week is it?', 'current_date_time'],
    ['I want to watch something on YouTube', 'open_website'],
    ['My laptop feels cluttered, where should I start?', 'pc_checkup'],
    ['Move my report to Downloads', 'move_file'],
    ['Create a folder for invoices', 'create_folder'],
  ])('offers the required capability for %s', (message, name) => {
    expect(selectToolNames(message)).toContain(name);
  });

  it('does not offer unrelated mutation tools to a read-only request', () => {
    const names = selectToolNames('How much free disk space do I have?');
    expect(names).toContain('check_disk_space');
    expect(names).not.toContain('move_file');
    expect(names).not.toContain('change_setting');
    expect(names.length).toBeLessThan(Object.keys(TOOLS).length / 2);
    expect(names.every((name) => Object.hasOwn(TOOLS, name))).toBe(true);
  });

  it('keeps settings, folder opening and process inspection separate from file search', () => {
    expect(selectToolNames('Stop showing me the hidden system files')).not.toContain('find_files');
    expect(selectToolNames('Which programs are eating my memory?')).not.toContain('list_open_apps');
    expect(selectToolNames('Bring up my pictures folder')).toEqual(['open_folder']);
    expect(selectToolNames('My computer is crawling')).toContain('system_status');
    expect(selectToolNames("What's 2 to the power of 20?")).toContain('calculate');
    expect(selectToolNames("What's hogging the space in my downloads?")).toEqual([
      'find_large_files',
    ]);
    expect(selectToolNames('Do I have the same pictures saved twice?')).toEqual([
      'find_duplicate_files',
    ]);
    expect(selectToolNames('Got any spreadsheets about the budget?')).toEqual(['find_files']);
    expect(selectToolNames('Write a story about duplicate files')).toEqual([]);
  });

  it('routes the current request without inheriting tool keywords from history', () => {
    const message =
      '<data>\nRecent PC activity and verified results:\nMoved a file and changed settings.\n</data>\n\nHow much free disk space do I have?';
    expect(selectToolNames(message)).toEqual(['check_disk_space']);
  });

  it('does not demand PC actions for definitions or explicitly negated requests', () => {
    expect(requiresToolUse('What is a duplicate file?')).toBe(false);
    expect(requiresToolUse("Don't open YouTube")).toBe(false);
    expect(requiresToolUse('I need to change my display settings')).toBe(true);
  });
});
