import { invoke } from '@tauri-apps/api/core';

export type FileOperation = {
  path: string;
  action: 'quarantine' | 'delete';
  sizeBytes: number;
};

export type Plan = {
  id: string;
  operations: FileOperation[];
  riskLevel: 'low' | 'medium' | 'high';
};

interface ApprovalViewProps {
  plan: Plan;
  onDecided: (approved: boolean) => void;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

/**
 * Read-only approval prompt. Signing happens in Rust after a genuine click is
 * observed by the input hook; this view never receives key material.
 */
export default function ApprovalView({ plan, onDecided }: ApprovalViewProps) {
  const handleApprove = async () => {
    // Notify Tauri backend that the user clicked approve.
    // The Rust `input_hook` ensures this was a physical click.
    try {
      await invoke('approve_plan', { planId: plan.id });
      onDecided(true);
    } catch (err) {
      console.error('Approval failed:', err);
    }
  };

  const handleReject = () => {
    onDecided(false);
  };

  return (
    <section className="approval-view" aria-labelledby="approval-title">
      <h2 id="approval-title" className={`risk-${plan.riskLevel}`}>
        Approval Required ({plan.riskLevel} risk)
      </h2>
      <p>The AI Agent proposes the following actions:</p>

      <ul className="file-operations-list">
        {plan.operations.map((op, idx) => (
          <li key={idx} className={`operation-item op-${op.action}`}>
            <span className="op-badge">{op.action}</span>
            <span className="op-path" title={op.path}>
              {op.path.length > 50 ? '...' + op.path.slice(-47) : op.path}
            </span>
            <span className="op-size">{formatBytes(op.sizeBytes)}</span>
          </li>
        ))}
      </ul>

      <div className="approval-actions">
        <button className="btn-reject" onClick={handleReject}>
          Reject
        </button>
        <button className="btn-approve" onClick={handleApprove}>
          Approve Plan
        </button>
      </div>
    </section>
  );
}
