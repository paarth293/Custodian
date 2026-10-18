"""Join the Qwen label-only comparator with the same native-effect baseline."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from mlhost.action_screen import baseline, compose, model_state
from mlhost.laya_compare import read_cases, summarize
from mlhost.typed_decisions import ALIGNMENTS, digest


def merge(cases: list[dict[str, Any]], qwen: dict[str, Any], case_bytes: bytes) -> dict[str, Any]:
    if qwen.get("case_file_sha256") != hashlib.sha256(case_bytes).hexdigest():
        raise ValueError("Qwen case file differs")
    rows = qwen.get("rows", [])
    if len(rows) != len(cases) or len({r["id"] for r in rows}) != len(cases):
        raise ValueError("Incomplete or duplicate Qwen results")
    by_id = {r["id"]: r for r in rows}
    merged = []
    raw_correct = 0
    for case in cases:
        row = by_id[case["id"]]
        if row["expected"] != case["expected"]:
            raise ValueError("Qwen reference labels differ")
        if row["status"] == "assessed" and row["alignment"] not in ALIGNMENTS:
            raise ValueError("Invalid Qwen decision")
        state_digest = digest(model_state(case["goal"], case["step"], case["action"]))
        result = compose(
            baseline(case["action"]),
            {
                "status": row["status"],
                "state_digest": state_digest,
                "decisions": {"alignment": {"label": row["alignment"]}},
            },
            state_digest,
        )
        raw_correct += row["alignment"] == case["expected"]["alignment"]
        merged.append(
            {
                "id": case["id"],
                "expected": case["expected"],
                "prediction": {**result["risk_flags"], "alignment": result["alignment"]},
                "elapsed_ms": row["elapsed_ms"],
                "status": row["status"],
            }
        )
    report = summarize(merged)
    report["raw_semantic_alignment_correct"] = raw_correct
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cases", type=Path, default=Path("training/laya/comparison.jsonl"))
    parser.add_argument("--laya-report", type=Path, required=True)
    parser.add_argument("--qwen-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    cases = read_cases(args.cases)
    report = json.loads(args.laya_report.read_text(encoding="utf-8"))
    if report.get("case_digest") != digest(cases) or report.get("status") != "development_only":
        raise ValueError("Laya case set differs or evaluation failed")
    qwen = json.loads(args.qwen_report.read_text(encoding="utf-8"))
    report["comparisons"]["rules_plus_qwen"] = merge(cases, qwen, args.cases.read_bytes())
    report["qwen_provenance"] = {k: v for k, v in qwen.items() if k != "rows"}
    report["deployment_approved"] = False
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
