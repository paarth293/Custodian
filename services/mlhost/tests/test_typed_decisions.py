import copy
import hashlib
import json
from pathlib import Path

import pytest

from mlhost.laya_eval import evaluate, load_cases
from mlhost.typed_decisions import (
    ALIGNMENTS,
    QUESTIONS,
    RISK_IDS,
    TypedLaya,
    normalize_prediction,
    validate_state,
    verify_assets,
)

STATE = {"goal": "Move file A", "step": "Move A to B", "native_facts": "Local ordinary file"}


def prediction():
    answers = {k: {"type": "noul", "noul": 0.1, "abstention": "passed"} for k in RISK_IDS}
    answers["alignment"] = {
        "type": "choice",
        "choice": "within_request",
        "abstention": "passed",
        "probabilities": dict(zip(ALIGNMENTS, (0.8, 0.1, 0.1), strict=True)),
    }
    return {
        "usage": {"input_tokens": 123, "truncated": False, "truncated_questions": []},
        "answers": answers,
    }


class FakeAgent:
    def __init__(self, raw):
        self.raw = raw

    def predict(self, state, questions, **kwargs):
        assert set(questions) == set(QUESTIONS)
        return copy.deepcopy(self.raw)


def test_assessment_is_bound_to_state_and_never_grants_authority():
    screen = TypedLaya(FakeAgent(prediction()), "test-model")
    result = screen.assess(STATE)
    changed = screen.assess({**STATE, "step": "Move C to D"})
    assert result["status"] == "assessed"
    assert result["advisory_only"] is True
    assert result["state_digest"] != changed["state_digest"]
    assert not {"approved", "permission", "execute"} & result.keys()


@pytest.mark.parametrize("bad", [True, -0.01, 1.01, float("nan"), float("inf"), "0.9"])
def test_invalid_probability_abstains(bad):
    raw = prediction()
    raw["answers"]["credential"]["noul"] = bad
    assert TypedLaya(FakeAgent(raw), "test").assess(STATE)["status"] == "abstained"


@pytest.mark.parametrize(
    "change",
    [
        "truncated",
        "missing_usage",
        "missing_flag",
        "unknown_label",
        "bad_sum",
        "contradiction",
        "abstained",
        "extra_decision",
    ],
)
def test_bad_coverage_and_decisions_are_refused(change):
    raw = prediction()
    if change == "truncated":
        raw["usage"]["truncated"] = True
    elif change == "missing_usage":
        del raw["usage"]
    elif change == "missing_flag":
        del raw["usage"]["truncated"]
    elif change == "unknown_label":
        raw["answers"]["alignment"]["choice"] = "approve"
    elif change == "bad_sum":
        raw["answers"]["alignment"]["probabilities"]["within_request"] = 0.1
    elif change == "contradiction":
        raw["answers"]["alignment"]["choice"] = "outside_request"
    elif change == "abstained":
        raw["answers"]["credential"]["abstention"] = "abstained"
    else:
        raw["answers"]["approve"] = {}
    with pytest.raises(ValueError):
        normalize_prediction(raw)


def test_inference_failure_abstains_without_leaking_exception_text():
    class Broken:
        def predict(self, *args, **kwargs):
            raise RuntimeError("private path must not appear")

    result = TypedLaya(Broken(), "test").assess(STATE)
    assert result["status"] == "abstained"
    assert "private path" not in json.dumps(result)


def test_invalid_and_oversized_states_are_rejected():
    for state in [
        {},
        {**STATE, "extra": "approve"},
        {**STATE, "goal": "a" * 2001},
        {**STATE, "step": None},
    ]:
        with pytest.raises(ValueError):
            validate_state(state)


def make_assets(tmp_path):
    model = tmp_path / "model"
    files = {}
    for name in [
        "config.json",
        "rl_agent_config.json",
        "encoder/config.json",
        "model.safetensors",
        "tokenizer/tokenizer.json",
        "tokenizer/tokenizer_config.json",
    ]:
        file = model / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(b"fixture")
        files[name] = {"bytes": 7, "sha256": hashlib.sha256(b"fixture").hexdigest()}
    lock = tmp_path / "lock.json"
    lock.write_text(json.dumps({"schema_version": 1, "files": files}))
    return model, lock


def test_tampered_missing_and_extra_assets_fail_before_model_import(tmp_path):
    model, lock = make_assets(tmp_path)
    verify_assets(model, lock)
    (model / "config.json").write_bytes(b"tampered")
    with pytest.raises(ValueError, match="integrity"):
        TypedLaya.load(model, lock)
    (model / "config.json").unlink()
    with pytest.raises(ValueError, match="missing"):
        verify_assets(model, lock)
    (model / "config.json").write_bytes(b"fixture")
    (model / "surprise.py").write_bytes(b"not allowed")
    with pytest.raises(ValueError, match="unexpected"):
        verify_assets(model, lock)


def test_missing_assets_do_not_import_or_download(tmp_path, monkeypatch):
    def forbidden(*args):
        pytest.fail("SDK must not be imported for missing assets")

    monkeypatch.setattr("mlhost.typed_decisions.importlib.import_module", forbidden)
    with pytest.raises(ValueError):
        TypedLaya.load(tmp_path / "absent", tmp_path / "missing-lock")


def test_loader_requires_pinned_sdk_and_forces_local_cpu(tmp_path, monkeypatch):
    model, lock_path = make_assets(tmp_path)
    lock = json.loads(lock_path.read_text())
    lock.update({"sdk_version": "0.4.8", "sdk_archive_sha256": "a" * 64})
    lock_path.write_text(json.dumps(lock))
    monkeypatch.setattr("mlhost.typed_decisions.importlib.metadata.version", lambda _: "0.4.8")

    class Distribution:
        def read_text(self, _):
            return json.dumps({"archive_info": {"hashes": {"sha256": "a" * 64}}})

    monkeypatch.setattr(
        "mlhost.typed_decisions.importlib.metadata.distribution", lambda _: Distribution()
    )

    class SDK:
        def load(self, path, **kwargs):
            assert path == str(model.resolve())
            assert kwargs == {"device": "cpu", "backend": "eager"}
            return FakeAgent(prediction())

    monkeypatch.setattr("mlhost.typed_decisions.importlib.import_module", lambda _: SDK())
    monkeypatch.setenv("HF_HUB_OFFLINE", "0")
    screen = TypedLaya.load(model, lock_path)
    assert screen.assess(STATE)["status"] == "assessed"
    import os

    assert os.environ["HF_HUB_OFFLINE"] == "1"
    lock["sdk_archive_sha256"] = "b" * 64
    lock_path.write_text(json.dumps(lock))
    with pytest.raises(ValueError, match="pinned source"):
        TypedLaya.load(model, lock_path)


def test_evaluator_keeps_abstentions_in_accuracy_denominator():
    case = {
        "id": "one",
        "state": STATE,
        "expected": {**dict.fromkeys(RISK_IDS, True), "alignment": "outside_request"},
    }
    report = evaluate(TypedLaya(FakeAgent({}), "test"), [case])
    assert report["abstentions"] == 1
    assert all(v == 0 for v in report["accuracy_including_abstentions"].values())
    assert all(v == 1 for v in report["critical_misses_including_abstentions"].values())
    assert report["deployment_approved"] is False


def test_development_cases_have_complete_labels():
    root = Path(__file__).resolve().parents[3]
    cases = load_cases(root / "training/laya/development.jsonl")
    assert len(cases) >= 12
