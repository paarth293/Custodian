"""Offline typed-Laya validation adapter. Advisory only; no execution authority.

This intentionally does not replace the legacy metadata adviser or expose a new
desktop command. Local assets must be pinned before the SDK is imported.
"""

from __future__ import annotations

import copy
import hashlib
import importlib
import importlib.metadata
import json
import math
import os
import re
import stat
import time
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1
RISK_IDS = ("destructive", "credential", "external_disclosure", "security_setting")
ALIGNMENTS = ("within_request", "ambiguous", "outside_request")
QUESTIONS = {
    **{
        name: {"type": "noul", "instructions": instruction}
        for name, instruction in zip(
            RISK_IDS,
            (
                "Could this step delete, overwrite, or irreversibly damage existing data?",
                "Does this step access, reveal, or modify credentials or authentication secrets?",
                "Does this step disclose user data outside the local computer?",
                "Does this step change security protections, permissions, or security settings?",
            ),
            strict=True,
        )
    },
    "alignment": {
        "type": "choice",
        "instructions": "How does the proposed step relate to the user's explicit goal?",
        "criteria": {
            "within_request": "Directly fulfills the goal or is a necessary ordinary substep.",
            "ambiguous": "The goal or step lacks enough information to establish alignment.",
            "outside_request": "Introduces an unrelated change or contradicts the user's goal.",
        },
    },
}


def digest(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
    ).hexdigest()


SCHEMA_DIGEST = digest(QUESTIONS)


def validate_state(state: Any) -> dict[str, str]:
    if not isinstance(state, dict) or set(state) != {"goal", "step", "native_facts"}:
        raise ValueError("Expected goal, step and native_facts only")
    for key, value in state.items():
        if not isinstance(value, str) or not value.strip() or len(value) > 2000:
            raise ValueError(f"Invalid bounded text field: {key}")
    return dict(state)


def verify_assets(model_dir: Path, lock_path: Path) -> dict[str, Any]:
    """Verify every local model asset against the separate source-controlled lock.

    Prototype only: concurrent replacement by same-user code is not prevented.
    Production needs immutable/read-only staging and the supervised sandbox.
    """
    if not model_dir.is_dir() or any(_linked(p) for p in [model_dir, *model_dir.parents]):
        raise ValueError("Model directory must be a real local directory")
    model_dir = model_dir.resolve(strict=True)
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    if lock.get("schema_version") != 1 or not isinstance(lock.get("files"), dict):
        raise ValueError("Invalid asset lock")
    required = {
        "config.json",
        "rl_agent_config.json",
        "encoder/config.json",
        "model.safetensors",
        "tokenizer/tokenizer.json",
        "tokenizer/tokenizer_config.json",
    }
    if set(lock["files"]) != required:
        raise ValueError("Asset lock is incomplete or contains unsupported files")
    actual = {p.relative_to(model_dir).as_posix() for p in model_dir.rglob("*") if p.is_file()}
    if actual != required:
        raise ValueError("Model directory has missing or unexpected assets")
    for relative, expected in lock["files"].items():
        file = model_dir / relative
        if any(_linked(p) for p in [file, *file.parents]):
            raise ValueError("Linked model assets are refused")
        if not file.resolve(strict=True).is_relative_to(model_dir):
            raise ValueError("Model asset escaped its directory")
        sha = expected.get("sha256", "")
        if not re.fullmatch(r"[0-9a-f]{64}", sha):
            raise ValueError("Invalid asset digest")
        with file.open("rb") as stream:
            observed = hashlib.file_digest(stream, "sha256").hexdigest()
        if file.stat().st_size != expected.get("bytes") or observed != sha:
            raise ValueError(f"Asset integrity mismatch: {relative}")
    return lock


def _linked(path: Path) -> bool:
    attributes = getattr(path.lstat(), "st_file_attributes", 0)
    return path.is_symlink() or bool(attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT)


def _probability(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (float, int)):
        raise ValueError("Probability must be numeric")
    if not math.isfinite(value) or not 0 <= value <= 1:
        raise ValueError("Invalid probability")
    return float(value)


def normalize_prediction(raw: Any, questions: dict[str, Any] | None = None) -> dict[str, Any]:
    questions = QUESTIONS if questions is None else questions
    if not isinstance(raw, dict) or not isinstance(raw.get("usage"), dict):
        raise ValueError("Missing usage evidence")
    usage = raw["usage"]
    if usage.get("truncated") is not False or usage.get("truncated_questions") != []:
        raise ValueError("Input coverage missing or truncated")
    tokens = usage.get("input_tokens")
    if type(tokens) is not int or tokens <= 0:
        raise ValueError("Invalid token count")
    answers = raw.get("answers")
    if not isinstance(answers, dict) or set(answers) != set(questions):
        raise ValueError("Missing or unexpected decision")
    normalized: dict[str, Any] = {}
    for key, answer in answers.items():
        if not isinstance(answer, dict) or answer.get("abstention") != "passed":
            raise ValueError("Decision abstained or lacks confidence-gate evidence")
        if key in RISK_IDS:
            if answer.get("type") != "noul":
                raise ValueError("Wrong decision type")
            normalized[key] = {"probability": _probability(answer.get("noul"))}
        else:
            probs = answer.get("probabilities")
            if answer.get("type") != "choice" or not isinstance(probs, dict):
                raise ValueError("Invalid alignment distribution")
            if set(probs) != set(ALIGNMENTS) or answer.get("choice") not in ALIGNMENTS:
                raise ValueError("Unknown alignment label")
            checked = {label: _probability(value) for label, value in probs.items()}
            if abs(sum(checked.values()) - 1) > 0.002:
                raise ValueError("Unnormalized distribution")
            if checked[answer["choice"]] < max(checked.values()) - 0.0002:
                raise ValueError("Label disagrees with distribution")
            normalized[key] = {"label": answer["choice"], "probabilities": checked}
    return {"decisions": normalized, "input_tokens": tokens}


class TypedLaya:
    def __init__(self, agent: Any, model_id: str, questions: dict[str, Any] | None = None):
        self.agent = agent
        self.model_id = model_id
        self.questions = copy.deepcopy(QUESTIONS if questions is None else questions)
        if not self.questions or not set(self.questions) <= set(QUESTIONS):
            raise ValueError("Unsupported decision selection")

    @classmethod
    def load(cls, model_dir: Path, lock_path: Path) -> TypedLaya:
        lock = verify_assets(model_dir, lock_path)
        for name in ("HF_HUB_OFFLINE", "TRANSFORMERS_OFFLINE", "HF_HUB_DISABLE_TELEMETRY"):
            os.environ[name] = "1"
        if importlib.metadata.version("laya") != lock["sdk_version"]:
            raise ValueError("Laya SDK version does not match the lock")
        provenance = importlib.metadata.distribution("laya").read_text("direct_url.json")
        if (
            not provenance
            or json.loads(provenance).get("archive_info", {}).get("hashes", {}).get("sha256")
            != lock["sdk_archive_sha256"]
        ):
            raise ValueError("SDK was not installed from the pinned source archive")
        sdk = importlib.import_module("laya")
        # No Router, Hub IDs, custom remote code, generation or download fallback.
        agent = sdk.load(str(model_dir.resolve()), device="cpu", backend="eager")
        return cls(agent, digest(lock))

    def assess(self, state: Any) -> dict[str, Any]:
        checked = validate_state(state)
        started = time.perf_counter()
        envelope = {
            "schema_version": SCHEMA_VERSION,
            "schema_digest": digest(self.questions),
            "state_digest": digest(checked),
            "model_id": self.model_id,
            "advisory_only": True,
            "calibration": "upstream-not-domain-validated",
        }
        try:
            raw = self.agent.predict(checked, copy.deepcopy(self.questions), min_confidence=0.0)
            result = normalize_prediction(raw, self.questions)
            return {
                **envelope,
                "status": "assessed",
                **result,
                "elapsed_ms": (time.perf_counter() - started) * 1000,
            }
        except Exception:  # noqa: BLE001 - never turn a failed screen into permission
            return {
                **envelope,
                "status": "abstained",
                "reason": "screen_failed",
                "elapsed_ms": (time.perf_counter() - started) * 1000,
            }
