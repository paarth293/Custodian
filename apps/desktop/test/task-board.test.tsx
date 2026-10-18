import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { ActiveTask } from '../src/components/ChatPanel';
import TaskBoard from '../src/components/TaskBoard';

const task = (): ActiveTask => ({
  id: 'fixture',
  description: 'Verified collection',
  state: 'running',
  execution: {
    program: {
      goal: 'Preserve originals',
      constraints: ['Pause after the bounded step'],
      steps: [
        { title: 'Copy', kind: 'write' },
        { title: 'Verify copies', kind: 'verify' },
      ],
    },
    outputs: { '0': { receipts: ['sealed-receipt'] } },
    skipped: [],
    in_flight: 1,
    pause_requested: false,
    paused: false,
    verified: false,
    failure: null,
  },
});
const render = (value: ActiveTask) =>
  renderToStaticMarkup(
    <TaskBoard activeTasks={[value]} onCancelTask={() => {}} onPauseTask={() => {}} />,
  );
describe('Task evidence', () => {
  it('distinguishes recorded effects from verified completion', () => {
    const html = render(task());
    expect(html).toContain('1 of 2 steps recorded');
    expect(html).toContain('Verify copies');
    expect(html).toContain('running');
    expect(html).not.toContain('Completion checked');
    expect(html).toContain('Pause after step');
  });
  it('shows pause acknowledgment separately from the request', () => {
    const value = task();
    value.execution!.pause_requested = true;
    expect(render(value)).toContain('Pause requested');
    value.execution!.paused = true;
    expect(render(value)).toContain('Paused');
    expect(render(value)).toContain('Resume');
  });
  it('keeps partial effects and the failure visible without offering resume', () => {
    const value = task();
    value.state = 'partial';
    value.execution!.failure = {
      class: 'missing-authority',
      detail: 'Permission changed <script>',
    };
    const html = render(value);
    expect(html).toContain('Stopped; review saved effects');
    expect(html).toContain('1 saved receipt references');
    expect(html).toContain('Permission changed &lt;script&gt;');
    expect(html).not.toContain('>Resume<');
    expect(html).not.toContain('>Cancel<');
  });
});
