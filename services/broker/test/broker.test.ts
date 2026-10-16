import { describe, expect, it } from 'vitest';
import { BudgetManager } from '../src/budgets';
import { classifyAction } from '../src/classification';
import { checkDlp } from '../src/dlp';
import { BrowserBroker } from '../src/index';
import { interceptRequest } from '../src/interception';
import { createReceipt } from '../src/receipts';

describe('BrowserBroker', () => {
  it('never claims execution for the unavailable browser and rejects unknown actions', async () => {
    const broker = new BrowserBroker();
    await expect(broker.performAction('navigate', { url: 'https://example.com' })).rejects.toThrow(
      'E_UNAVAILABLE',
    );
    await expect(broker.performAction('unknown', {})).rejects.toThrow('E_POLICY');
  });

  it('refuses to start without the Chromium sandbox', () => {
    expect(() => new BrowserBroker().start(['--no-sandbox'])).toThrow('E_POLICY');
    expect(() => new BrowserBroker().start([])).toThrow('E_UNAVAILABLE');
  });
});

describe('policies', () => {
  it('hands sensitive actions back to the user', () => {
    expect(classifyAction('payment')).toBe('handover');
    expect(classifyAction('scroll')).toBe('proceed');
  });

  it('blocks payloads containing protected tokens', () => {
    expect(checkDlp('my ssn is 123', ['123'])).toBe('blocked');
    expect(checkDlp('hello', ['123', ''])).toBe('clean');
  });

  it('blocks plain HTTP under the any_https policy', () => {
    expect(interceptRequest('http://example.com', 'any_https')).toBe('block');
    expect(interceptRequest('https://example.com', 'any_https')).toBe('allow');
  });

  it('truncates receipt metadata', () => {
    expect(createReceipt('download', { a: 'x'.repeat(500) }).length).toBeLessThan(300);
  });
});

describe('BudgetManager', () => {
  it('exhausts after its minutes run out', () => {
    const budget = new BudgetManager(2);
    budget.tick();
    expect(budget.exhausted).toBe(false);
    budget.tick();
    budget.tick();
    expect(budget.exhausted).toBe(true);
    expect(budget.minutes).toBe(0);
  });
});
