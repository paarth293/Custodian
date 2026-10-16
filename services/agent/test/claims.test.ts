import { describe, expect, it } from 'vitest';
import { actionNarration, claimsAction, claimsDeletion, claimsMutation } from '../src/claims';

describe('action narration', () => {
  it('does not turn an approval proposal into a success claim', () => {
    const reply = actionNarration(['Waiting for the person: a card asks to change dark mode.']);
    expect(reply).toContain('has not run');
    expect(reply).not.toMatch(/is now dark|turned on|changed/i);
  });
  it('does not claim success after a failure or an unrelated successful action', () => {
    const reply = actionNarration(['Opened Notepad.', 'I could not move the file.']);
    expect(reply).toContain('action result is shown above');
    expect(reply).not.toMatch(/moved|completed successfully/);
  });
});

describe('claimsAction', () => {
  it('catches the false claims a small model made (from real evaluation logs)', () => {
    for (const text of [
      "I've closed Spotify for you. Do you want to save any work?",
      'I found your resume document in the Documents folder.',
      "I've found some duplicate files in your Documents.",
      'The calculator has been successfully opened! 😊',
      'Notepad is now open.',
      'I have deleted the old installers.',
      'I just set a reminder for 4 PM.',
      "I'll open Notepad for you right away.",
      'I will now search the web for cheap flights.',
      "I'll check your system's performance and temperature.",
    ]) {
      expect(claimsAction(text), text).toBe(true);
    }
  });

  it('leaves plain answers and offers alone', () => {
    for (const text of [
      'A duplicate file is a copy of another file.',
      'I can help you find your resume. Where did you last see it?',
      "Let's check the free space on your C drive.",
      'Would you like me to open Notepad?',
      'I think the best option is the cheaper one.',
      'Sure! What should I remind you about?',
      "I'll need the file's name to find it.",
      "I'll help you get organized.",
      'Let me know if you want anything else.',
    ]) {
      expect(claimsAction(text), text).toBe(false);
    }
  });
});

describe('claimsMutation', () => {
  it('catches state-changing mutations', () => {
    expect(claimsMutation("I've closed Spotify for you.")).toBe(true);
    expect(claimsMutation('The calculator has been successfully opened! 😊')).toBe(true);
    expect(claimsMutation('Notepad is now open.')).toBe(true);
    expect(claimsMutation('I have deleted the old installers.')).toBe(true);
    expect(claimsMutation("I'll open Notepad for you right away.")).toBe(true);
  });

  it('does not trigger on read-only search or inspection claims', () => {
    expect(claimsMutation('I found your resume document in the Documents folder.')).toBe(false);
    expect(claimsMutation("I've found some duplicate files in your Documents.")).toBe(false);
    expect(claimsMutation("I'll check your system's performance and temperature.")).toBe(false);
    expect(claimsMutation('I checked your disk space.')).toBe(false);
  });
});

describe('claimsDeletion', () => {
  it('detects deletion claims', () => {
    expect(claimsDeletion('I have deleted the old installers.')).toBe(true);
    expect(claimsDeletion('The temp files were deleted.')).toBe(true);
    expect(claimsDeletion('Your logs have been removed.')).toBe(true);
  });

  it('leaves other actions alone', () => {
    expect(claimsDeletion("I've closed Spotify for you.")).toBe(false);
    expect(claimsDeletion('Notepad is now open.')).toBe(false);
  });
});
