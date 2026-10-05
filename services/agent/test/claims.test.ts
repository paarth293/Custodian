import { describe, expect, it } from 'vitest';
import { claimsAction } from '../src/claims';

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
