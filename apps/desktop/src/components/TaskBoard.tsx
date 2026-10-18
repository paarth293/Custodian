import type { ActiveTask } from './ChatPanel';

export default function TaskBoard({
  activeTasks,
  onCancelTask,
  onPauseTask,
  onInspectWorkflow,
}: {
  activeTasks: ActiveTask[];
  onCancelTask: (id: string) => void;
  onPauseTask: (id: string, paused: boolean) => void;
  onInspectWorkflow?: () => void;
}) {
  return (
    <>
      {activeTasks.length > 0 && (
        <aside className="active-tasks-board" aria-label="Background tasks">
          <div className="active-tasks-header">Tasks ({activeTasks.length})</div>
          {onInspectWorkflow &&
            activeTasks.some((task) =>
              task.execution?.program.steps.some((step) => step.capability === 'collection-copy'),
            ) && (
              <div>
                <button type="button" onClick={onInspectWorkflow}>
                  Inspect latest collection
                </button>
                <small>
                  For recovery or undo, inspect the latest saved collection first. Changed files are
                  retained.
                </small>
              </div>
            )}
          <ul className="active-tasks-list">
            {activeTasks.map((task) => (
              <li key={task.id} className="active-task-item">
                <div className="task-desc" title={task.description}>
                  {task.description}
                  {task.progress && <small>{task.progress}</small>}
                  {task.execution && (
                    <details>
                      <summary>
                        {task.execution.paused
                          ? 'Paused'
                          : task.execution.pause_requested
                            ? 'Pause requested'
                            : 'Execution evidence'}
                        {' · '}
                        {Object.keys(task.execution.outputs).length} of{' '}
                        {task.execution.program.steps.length} steps recorded
                      </summary>
                      <p>{task.execution.program.goal}</p>
                      <ol>
                        {task.execution.program.steps.map((step, index) => (
                          <li key={index}>
                            {step.title}:{' '}
                            {task.execution?.outputs[String(index)]
                              ? 'recorded'
                              : task.execution?.skipped.includes(index)
                                ? 'skipped'
                                : task.execution?.in_flight === index
                                  ? 'running'
                                  : 'waiting'}
                            {step.kind === 'app' &&
                              ' (adapter capability; real app support depends on registration)'}
                          </li>
                        ))}
                      </ol>
                      {task.execution.program.constraints.map((text) => (
                        <p key={text}>{text}</p>
                      ))}
                      {task.execution.failure && (
                        <p role="status">
                          {task.execution.failure.class}: {task.execution.failure.detail}
                        </p>
                      )}
                      <p>
                        {Object.values(task.execution.outputs).reduce(
                          (count, output) => count + output.receipts.length,
                          0,
                        )}{' '}
                        saved receipt references.
                      </p>
                    </details>
                  )}
                </div>
                {task.execution && task.state === 'running' && (
                  <button
                    type="button"
                    onClick={() => void onPauseTask(task.id, !task.execution?.pause_requested)}
                  >
                    {task.execution.pause_requested ? 'Resume' : 'Pause after step'}
                  </button>
                )}
                {['recovery-required', 'partial', 'failed', 'completed', 'cancelled'].includes(
                  task.state,
                ) ? (
                  <span role="status">
                    {task.state === 'completed'
                      ? 'Completion checked'
                      : 'Stopped; review saved effects'}
                  </span>
                ) : (
                  <button
                    type="button"
                    className="btn-cancel-task"
                    onClick={() => void onCancelTask(task.id)}
                    aria-label={`Cancel ${task.description}`}
                  >
                    Cancel
                  </button>
                )}
              </li>
            ))}
          </ul>
        </aside>
      )}
    </>
  );
}
