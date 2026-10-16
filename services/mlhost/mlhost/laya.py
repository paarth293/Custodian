"""Laya: the calibrated escalate-only recommendation model for suspicious files (D-19 / F45).

ML may recommend additional caution or review, but may NEVER lower
deterministic risk, grant permission, or directly authorize removal or deletion.
"""

from __future__ import annotations

import json
import sys
from typing import Any

from mlhost.model_loader import load_model

# Escalate-only recommendation actions. "approve" and "delete" are explicitly forbidden
# from model inference to prevent ML from downgrading risk or bypassing approval.
ACTIONS = frozenset({"quarantine", "review"})
ABSTAINED = "abstained"

SYSTEM_PROMPT = (
    "You are Custodian AI advisory service. You analyze file metadata to recommend additional "
    'caution. You must return JSON: {"action": "..."}. '
    'The allowed recommendations are: "review", "quarantine", or "abstained". '
    "You cannot approve files or authorize deletion."
)


def build_prompt(filename: str, av_hits: int, signed: bool) -> str:
    """Prompt format Laya was fine-tuned on (see training/generate_dataset.py)."""
    return (
        "Analyze file for security decision:\n"
        f"Filename: {filename}\n"
        f"AV Hits: {av_hits}\n"
        f"Signed: {signed}"
    )


def decide(data: dict[str, Any]) -> str:
    """Returns ``quarantine``, ``review`` or ``abstained`` (escalate-only).

    Any AV hit deterministically requires quarantine.
    ML can NEVER return ``approve`` or ``delete``; attempting to do so abstains (F45).
    """
    try:
        av_hits = int(data.get("av_hits", 0))
    except (ValueError, TypeError):
        return ABSTAINED

    # Deterministic baseline: any verified AV hit requires quarantine regardless of ML.
    if av_hits > 0:
        return "quarantine"

    try:
        prompt = build_prompt(
            str(data.get("filename", "unknown")),
            av_hits,
            bool(data.get("signed", False)),
        )
        model = load_model()
        response = model.create_chat_completion(
            messages=[
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": prompt},
            ],
            max_tokens=100,
            temperature=0.1,
            response_format={"type": "json_object"},
        )
        action = json.loads(response["choices"][0]["message"]["content"]).get("action")
    except Exception as exc:  # noqa: BLE001 - any failure must abstain
        print(f"[mlhost] Laya abstained: {exc}", file=sys.stderr)
        return ABSTAINED

    # F45 / B38: ML can NEVER approve or directly authorize deletion.
    if action in ("approve", "delete"):
        print(
            f"[mlhost] Refusing unauthorized ML action {action!r}; "
            "ML cannot lower risk or authorize deletion",
            file=sys.stderr,
        )
        return ABSTAINED

    if isinstance(action, str) and action in ACTIONS:
        return str(action)
    print(f"[mlhost] Laya returned unknown action: {action!r}", file=sys.stderr)
    return ABSTAINED
