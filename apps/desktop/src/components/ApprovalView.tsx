interface ApprovalViewProps {
  planHash: string;
}

/**
 * Read-only approval prompt. Signing happens in Rust after a genuine click is
 * observed by the input hook; this view never receives key material.
 */
export default function ApprovalView({ planHash }: ApprovalViewProps) {
  return (
    <section className="approval-view" aria-labelledby="approval-title">
      <h2 id="approval-title">Approval required</h2>
      <p>
        Plan <code>{planHash}</code> is waiting for your approval.
      </p>
    </section>
  );
}
