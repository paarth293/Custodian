import { describe, expect, it } from 'vitest';
import { mightBeToolCall, parseWrittenCall } from '../src/salvage';

const known = new Set(['check_disk_space', 'find_large_files', 'find_files', 'open_app']);

describe('parseWrittenCall', () => {
  it('recovers the calls small models wrote out (from real evaluation logs)', () => {
    expect(
      parseWrittenCall('{"name": "check_disk_space", "arguments": {}}\n\n</tool_call>', known),
    ).toEqual({ name: 'check_disk_space', args: {} });
    expect(
      parseWrittenCall(
        '<tools>\n{"name": "find_large_files", "arguments": {"folders": ["Downloads"]}}\n</tools>',
        known,
      ),
    ).toEqual({ name: 'find_large_files', args: { folders: ['Downloads'] } });
    expect(
      parseWrittenCall(
        '<tools>\n{"type": "function", "function": "check_disk_space", "arguments": {"drive": "C"}}\n</tools>',
        known,
      ),
    ).toEqual({ name: 'check_disk_space', args: { drive: 'C' } });
    expect(
      parseWrittenCall(
        '<tool_call>\n{"name": "open_app", "arguments": {"app": "settings"}}\n</tool_call>\n',
        known,
      ),
    ).toEqual({ name: 'open_app', args: { app: 'settings' } });
  });

  it('accepts fences, nested functions and string arguments', () => {
    expect(
      parseWrittenCall('```json\n{"name": "find_files", "arguments": {"name": "cv"}}\n```', known),
    ).toEqual({ name: 'find_files', args: { name: 'cv' } });
    expect(
      parseWrittenCall('{"function": {"name": "find_files", "arguments": {"name": "tax"}}}', known),
    ).toEqual({ name: 'find_files', args: { name: 'tax' } });
    expect(
      parseWrittenCall('{"name": "find_files", "arguments": "{\\"name\\": \\"x\\"}"}', known),
    ).toEqual({ name: 'find_files', args: { name: 'x' } });
  });

  it('refuses anything that is not exactly a call to a known tool', () => {
    for (const text of [
      'Your C: drive has 41 GB free.',
      '{"name": "run_command", "arguments": {"cmd": "del"}}',
      '{"name": "find_files", "arguments": [1, 2]}',
      '{"name": "find_files"} and then some words',
      '<tool_call>{not json}</tool_call>',
      '{"arguments": {}}',
      '[]',
      '',
    ]) {
      expect(parseWrittenCall(text, known), text).toBeNull();
    }
  });
});

describe('mightBeToolCall', () => {
  it('holds back replies that start like a call, and only those', () => {
    expect(mightBeToolCall('<tool_call>')).toBe(true);
    expect(mightBeToolCall('  {"name"')).toBe(true);
    expect(mightBeToolCall('```json')).toBe(true);
    expect(mightBeToolCall('Sure! Your disk')).toBe(false);
  });
});
