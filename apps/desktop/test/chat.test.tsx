import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { appendReply, bubbleText, decideCard } from '../src/chat/events';
import ApprovalView from '../src/components/ApprovalView';
import ChatPanel, { type Message } from '../src/components/ChatPanel';

const card = {
  id: 'c1',
  title: 'Search the web?',
  detail: 'Search Google for "weather in Delhi" in your browser. Only these words are sent.',
  confirm: 'Search',
};

describe('appendReply', () => {
  it('creates the reply on the first chunk and extends it afterwards', () => {
    let messages: Message[] = [];
    messages = appendReply(messages, 'r1', 'Hello');
    messages = appendReply(messages, 'r1', ' there');
    expect(messages).toEqual([{ id: 'r1', role: 'assistant', content: 'Hello there' }]);
  });
});

describe('bubbleText', () => {
  it('keeps the first sentence and stays short', () => {
    expect(
      bubbleText(
        'I freed 3.4 GB by deleting 12 duplicate copies from my safe place. The originals are all still where they were.',
      ),
    ).toBe('I freed 3.4 GB by deleting 12 duplicate copies from my safe place.');
    expect(bubbleText('Hi!')).toBe('Hi!');
    const long = bubbleText('x'.repeat(200));
    expect(long.length).toBeLessThanOrEqual(70);
    expect(long.endsWith('…')).toBe(true);
  });
});

describe('decideCard', () => {
  it('records the answer once, and only on cards', () => {
    const messages: Message[] = [
      { id: 'c1', role: 'assistant', content: '', card },
      { id: 'm1', role: 'assistant', content: 'hi' },
    ];
    const allowed = decideCard(messages, 'c1', 'allowed');
    expect(allowed[0]?.decided).toBe('allowed');
    // A second click cannot change the answer.
    expect(decideCard(allowed, 'c1', 'declined')[0]?.decided).toBe('allowed');
    expect(decideCard(messages, 'm1', 'allowed')[1]?.decided).toBeUndefined();
  });
});

describe('ApprovalView', () => {
  const op = (path: string, action: 'delete' | 'quarantine') => ({ path, action, sizeBytes: 1024 });

  it('counts every file, not just the ones listed', () => {
    const html = renderToStaticMarkup(
      <ApprovalView
        plan={{
          id: 'p',
          riskLevel: 'medium',
          fileCount: 812,
          totalBytes: 1024 * 1024,
          operations: [op('C:\\t\\a.tmp', 'delete'), op('C:\\t\\b.tmp', 'delete')],
        }}
        onDecided={() => {}}
      />,
    );
    expect(html).toContain('812 files');
    expect(html).toContain('+810 more');
    expect(html).toContain('Delete files');
  });

  it('keeps the clean-up wording for moves to the safe place', () => {
    const html = renderToStaticMarkup(
      <ApprovalView
        plan={{ id: 'p', riskLevel: 'low', operations: [op('a.jpg', 'quarantine')] }}
        onDecided={() => {}}
      />,
    );
    expect(html).toContain('1 file');
    expect(html).toContain('Approve clean-up');
  });
});

describe('ChatPanel', () => {
  const render = (messages: Message[], isThinking = false) =>
    renderToStaticMarkup(
      <ChatPanel messages={messages} isThinking={isThinking} onSendMessage={() => {}} />,
    );

  it('shows a waiting card with both choices', () => {
    const html = render([{ id: 'c1', role: 'assistant', content: '', card }]);
    expect(html).toContain('Search the web?');
    expect(html).toContain('&quot;weather in Delhi&quot;');
    expect(html).toContain('>Not now<');
    expect(html).toContain('>Search<');
  });

  it('shows the answer instead of buttons once decided', () => {
    const html = render([{ id: 'c1', role: 'assistant', content: '', card, decided: 'declined' }]);
    expect(html).not.toContain('>Search<');
    expect(html).toContain('Not now');
  });

  it('marks tool results so they look different from the AI', () => {
    const html = render([
      { id: 'r', role: 'assistant', content: 'C: has 41 GB free.', result: true },
      { id: 't', role: 'assistant', content: 'Plenty of room!' },
    ]);
    expect(html).toContain('class="chat-message assistant result"');
    expect(html).toContain('class="chat-message assistant"');
  });

  it('never renders text as HTML', () => {
    const html = render([{ id: 'x', role: 'assistant', content: '<img src=x onerror=alert(1)>' }]);
    expect(html).not.toContain('<img');
    expect(html).toContain('&lt;img');
  });

  it('offers Stop while Cubby is working', () => {
    expect(render([], true)).toContain('>Stop<');
    expect(render([], false)).toContain('>Send<');
  });
});
