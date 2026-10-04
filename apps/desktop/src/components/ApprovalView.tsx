import { useState } from 'react';
import { call } from '../lib/tauri';

export type FileOperation = {
  path: string;
  /** The identical copy that stays where it is. */
  keep?: string;
  action: 'quarantine' | 'delete';
  sizeBytes: number;
};

export type Plan = {
  id: string;
  operations: FileOperation[];
  riskLevel: 'low' | 'medium' | 'high';
  summary?: string;
  totalBytes?: number;
};

interface ApprovalViewProps {
  plan: Plan;
  /** Called after the decision with the result to show in the chat. */
  onDecided: (approved: boolean, result: string) => void;
}

/** Items listed before collapsing the rest into "+N more". */
const VISIBLE_ITEMS = 50;

function formatBytes(bytes: number) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

/**
 * The approval card for a clean-up plan. The plan itself lives in Rust; this
 * view only sends the plan id back, so the webview can never inject paths.
 */
export default function ApprovalView({ plan, onDecided }: ApprovalViewProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const total = plan.totalBytes ?? plan.operations.reduce((sum, op) => sum + op.sizeBytes, 0);
  const hidden = plan.operations.length - VISIBLE_ITEMS;

  const approve = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await call<string>('approve_plan', { planId: plan.id });
      onDecided(true, result ?? 'Done.');
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };

  const reject = async () => {
    await call('reject_plan', { planId: plan.id }).catch(() => undefined);
    onDecided(false, "Okay, I won't change anything.");
  };

  return (
    <section className="approval-view" aria-labelledby="approval-title">
      <h2 id="approval-title" className={`risk-${plan.riskLevel}`}>
        Clean-up plan · {plan.operations.length} {plan.operations.length === 1 ? 'file' : 'files'} ·{' '}
        {formatBytes(total)}
      </h2>
      {plan.summary && <p>{plan.summary}</p>}

      <ul className="file-operations-list">
        {plan.operations.slice(0, VISIBLE_ITEMS).map((op) => (
          <li
            key={op.path}
            className={`operation-item op-${op.action}`}
            title={op.keep ? `${op.path}\nIdentical to (kept): ${op.keep}` : op.path}
          >
            <span className="op-badge">{op.action === 'quarantine' ? 'move' : op.action}</span>
            <span className="op-path">{fileName(op.path)}</span>
            <span className="op-size">{formatBytes(op.sizeBytes)}</span>
          </li>
        ))}
        {hidden > 0 && <li className="operation-more">+{hidden} more</li>}
      </ul>

      {error && (
        <p className="approval-error" role="alert">
          {error}
        </p>
      )}
      <div className="approval-actions">
        <button className="btn-reject" onClick={() => void reject()} disabled={busy}>
          Not now
        </button>
        <button className="btn-approve" onClick={() => void approve()} disabled={busy}>
          {busy ? 'Cleaning up…' : 'Approve clean-up'}
        </button>
      </div>
    </section>
  );
}
