from pathlib import Path

import pytest

from mlhost.action_screen import baseline, compose, model_state, validate_action
from mlhost.laya_compare import quantize_candidate, read_cases, reference_results, summarize
from mlhost.typed_decisions import QUESTIONS, TypedLaya, digest


def action(**changes):
    return {
        "tool": "copy",
        "source_id": "A",
        "selected_ids": ["A"],
        "target_kind": "ordinary",
        "overwrites": False,
        "references": "resolved",
        **changes,
    }


@pytest.mark.parametrize(
    ("tool", "flag"),
    [
        ("permanent_delete", "destructive"),
        ("upload", "external_disclosure"),
        ("read_credentials", "credential"),
        ("change_security", "security_setting"),
    ],
)
def test_known_effect_does_not_depend_on_model_or_names(tool, flag):
    native = baseline(action(tool=tool))
    assert native["risk_flags"][flag] is True
    semantic = {
        "status": "assessed",
        "state_digest": "current",
        "decisions": {"alignment": {"label": "within_request"}},
    }
    assert compose(native, semantic, "current")["risk_flags"][flag] is True


def test_generic_reader_cannot_hide_credential_target():
    assert baseline(action(tool="read_file", target_kind="credential"))["risk_flags"]["credential"]


def test_overwrite_and_security_config_changes_escalate():
    native = baseline(action(tool="copy", target_kind="security", overwrites=True))
    assert native["risk_flags"]["destructive"] is True
    assert native["risk_flags"]["security_setting"] is True


@pytest.mark.parametrize("changes", [{"source_id": "B"}, {"references": "unresolved"}])
@pytest.mark.parametrize("label", ["within_request", "ambiguous", "outside_request"])
def test_model_never_clears_native_alignment_restriction(changes, label):
    native = baseline(action(**changes))
    semantic = {
        "status": "assessed",
        "state_digest": "same",
        "decisions": {"alignment": {"label": label}},
    }
    assert compose(native, semantic, "same")["alignment"] == native["alignment"]


def test_unknown_tools_and_missing_source_stay_unknown():
    for facts in [
        action(tool="run_anything"),
        action(source_id=None),
        action(target_kind="unknown"),
    ]:
        native = baseline(facts)
        semantic = {
            "status": "assessed",
            "state_digest": "same",
            "decisions": {"alignment": {"label": "within_request"}},
        }
        assert compose(native, semantic, "same")["unknown"] is True
    assert all(v is None for v in baseline(action(tool="run_anything"))["risk_flags"].values())


@pytest.mark.parametrize(
    "result",
    [
        {"status": "abstained"},
        {
            "status": "assessed",
            "state_digest": "stale",
            "decisions": {"alignment": {"label": "within_request"}},
        },
        {
            "status": "assessed",
            "state_digest": "current",
            "decisions": {"alignment": {"label": "approve"}},
        },
    ],
)
def test_failed_stale_or_unknown_semantics_never_complete_a_screen(result):
    output = compose(baseline(action()), result, "current")
    assert output["alignment"] is None
    assert output["semantic_status"] == "unavailable"
    assert "approved" not in output


def test_facts_are_typed_and_do_not_include_expected_decisions():
    for bad in [
        action(overwrites="false"),
        action(selected_ids=["A", "A"]),
        {**action(), "safe": True},
    ]:
        with pytest.raises(ValueError):
            validate_action(bad)
    state = model_state("Copy selected file", "Copy A", action())
    assert set(state) == {"goal", "step", "native_facts"}
    assert "expected" not in state["native_facts"]


def test_selective_inference_requests_only_alignment():
    class Agent:
        def predict(self, state, questions, **kwargs):
            assert set(questions) == {"alignment"}
            return {
                "usage": {"input_tokens": 40, "truncated": False, "truncated_questions": []},
                "answers": {
                    "alignment": {
                        "type": "choice",
                        "abstention": "passed",
                        "choice": "within_request",
                        "probabilities": {
                            "within_request": 0.8,
                            "ambiguous": 0.1,
                            "outside_request": 0.1,
                        },
                    }
                },
            }

    state = model_state("Copy selected file", "Copy A", action())
    screen = TypedLaya(Agent(), "test", {"alignment": QUESTIONS["alignment"]})
    result = screen.assess(state)
    assert result["status"] == "assessed"
    assert result["state_digest"] == digest(state)
    assert set(result["decisions"]) == {"alignment"}


def test_unknown_baseline_is_not_counted_as_correct_or_safe():
    truth = {
        "destructive": False,
        "credential": False,
        "external_disclosure": True,
        "security_setting": False,
        "alignment": "outside_request",
    }
    report = summarize([{"prediction": dict.fromkeys(truth), "expected": truth, "elapsed_ms": 1}])
    assert all(n == 0 for n in report["correct_counts"].values())
    assert report["critical_misses_including_unknown"]["external_disclosure"] == 1
    assert report["unknown_counts"]["alignment"] == 1


def test_new_cases_are_complete_and_effect_baseline_matches_known_facts():
    cases = read_cases(Path(__file__).resolve().parents[3] / "training/laya/comparison.jsonl")
    assert len(cases) == 20
    for case in cases:
        result = baseline(case["action"])
        for key, value in result["risk_flags"].items():
            assert value == case["expected"][key], case["id"]


def test_encoder_quantization_preserves_reference_and_floating_head():
    from types import SimpleNamespace

    model = SimpleNamespace(encoder={"weights": "encoder"}, head={"weights": "floating"})
    screen = TypedLaya(SimpleNamespace(model=model), "reference")

    def quantize(encoder, layers, **kwargs):
        assert kwargs["inplace"] is True
        encoder["quantized"] = True
        return encoder

    torch = SimpleNamespace(
        ao=SimpleNamespace(quantization=SimpleNamespace(quantize_dynamic=quantize)),
        nn=SimpleNamespace(Linear=object),
        qint8="int8",
    )
    candidate = quantize_candidate(screen, torch, True)
    assert "quantized" not in screen.agent.model.encoder
    assert candidate.agent.model.encoder["quantized"] is True
    assert candidate.agent.model.head == screen.agent.model.head
    assert candidate.agent.model.head is not screen.agent.model.head


def test_reference_reuse_checks_model_schema_state_and_labels(tmp_path):
    import copy
    import json

    cases = read_cases(Path(__file__).resolve().parents[3] / "training/laya/comparison.jsonl")[:1]
    case = cases[0]
    schema = {"alignment": QUESTIONS["alignment"]}
    row = {
        "id": case["id"],
        "expected": case["expected"],
        "semantic": {
            "schema_digest": digest(schema),
            "model_id": "model",
            "state_digest": digest(model_state(case["goal"], case["step"], case["action"])),
        },
    }
    reference = {
        "status": "development_only",
        "case_digest": digest(cases),
        "comparisons": {"original": {"rows": [row]}},
    }
    path = tmp_path / "reference.json"
    path.write_text(json.dumps(reference))
    reference_results(path, cases, {"original": schema}, "model")
    for field in ("model_id", "schema_digest", "state_digest"):
        changed = copy.deepcopy(reference)
        changed["comparisons"]["original"]["rows"][0]["semantic"][field] = "different"
        path.write_text(json.dumps(changed))
        with pytest.raises(ValueError):
            reference_results(path, cases, {"original": schema}, "model")
