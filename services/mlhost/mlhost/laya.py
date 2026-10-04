"""Laya: the fine-tuned decision model for suspicious files."""

from __future__ import annotations

import json
import sys
from typing import Any

from mlhost.model_loader import load_model

ACTIONS = frozenset({"approve", "delete", "quarantine"})
ABSTAINED = "abstained"


def build_prompt(filename: str, av_hits: int, signed: bool) -> str:
    """Prompt format Laya was fine-tuned on (see training/generate_dataset.py)."""
    return (
        "Analyze file for security decision:\n"
        f"Filename: {filename}\n"
        f"AV Hits: {av_hits}\n"
        f"Signed: {signed}"
    )


def decide(data: dict[str, Any]) -> str:
    """Returns ``approve``, ``delete``, ``quarantine`` or ``abstained``.

    Any failure (missing model, invalid output) abstains so the caller falls
    back to asking the user.
    """
    try:
        prompt = build_prompt(
            str(data.get("filename", "unknown")),
            int(data.get("av_hits", 0)),
            bool(data.get("signed", False)),
        )
        model = load_model()
        response = model.create_chat_completion(
            messages=[
                {
                    "role": "system",
                    "content": "You are Custodian AI. You analyze file metadata and return exactly one action in JSON format: {'action': '...'}. The ONLY allowed actions are: 'approve', 'delete', or 'quarantine'. Do not use any other words."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            max_tokens=100,
            temperature=0.1,
            response_format={"type": "json_object"},
        )
        action = json.loads(response["choices"][0]["message"]["content"]).get("action")
    except Exception as exc:  # noqa: BLE001 - any failure must abstain
        print(f"[mlhost] Laya abstained: {exc}", file=sys.stderr)
        return ABSTAINED

    if action in ACTIONS:
        return str(action)
    print(f"[mlhost] Laya returned unknown action: {action!r}", file=sys.stderr)
    return ABSTAINED
