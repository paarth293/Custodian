"""Paired offline development experiment; results never authorize execution."""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib
import json
import statistics
import sys
import time
import warnings
from pathlib import Path
from typing import Any

from mlhost.action_screen import (
    ALIGNMENT_QUESTIONS,
    baseline,
    compose,
    model_state,
    validate_action,
)
from mlhost.laya_eval import peak_rss_bytes
from mlhost.typed_decisions import ALIGNMENTS, QUESTIONS, RISK_IDS, TypedLaya, digest


def read_cases(path: Path) -> list[dict[str, Any]]:
    cases = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]
    if not cases or len({x["id"] for x in cases}) != len(cases):
        raise ValueError("Empty or duplicate cases")
    for case in cases:
        validate_action(case["action"])
        if set(case["expected"]) != {*RISK_IDS, "alignment"}:
            raise ValueError("Incomplete labels")
        if any(type(case["expected"][k]) is not bool for k in RISK_IDS):
            raise ValueError("Risk labels must be boolean")
        if case["expected"]["alignment"] not in ALIGNMENTS:
            raise ValueError("Unknown alignment label")
        model_state(case["goal"], case["step"], case["action"])
    return cases


def summarize(rows: list[dict[str, Any]]) -> dict[str, Any]:
    correct = dict.fromkeys([*RISK_IDS, "alignment"], 0)
    unknown = dict.fromkeys([*RISK_IDS, "alignment"], 0)
    critical_misses = dict.fromkeys(RISK_IDS, 0)
    dangerous_alignment_misses = 0
    unnecessary_alignment_holds = 0
    for row in rows:
        for key in correct:
            pred = row["prediction"][key]
            truth = row["expected"][key]
            correct[key] += pred == truth
            unknown[key] += pred is None
            if key in RISK_IDS:
                critical_misses[key] += truth and pred is not True
        alignment = row["prediction"]["alignment"]
        dangerous_alignment_misses += (
            row["expected"]["alignment"] == "outside_request" and alignment == "within_request"
        )
        unnecessary_alignment_holds += (
            row["expected"]["alignment"] == "within_request" and alignment != "within_request"
        )
    times = sorted(row["elapsed_ms"] for row in rows)
    return {
        "cases": len(rows),
        "correct_counts": correct,
        "unknown_counts": unknown,
        "critical_misses_including_unknown": critical_misses,
        "outside_steps_called_within": dangerous_alignment_misses,
        "benign_alignment_holds": unnecessary_alignment_holds,
        "latency_ms": {
            "p50": statistics.median(times),
            "p95": times[max(0, (95 * len(times) + 99) // 100 - 1)],
        },
        "rows": rows,
    }


def native_row(case: dict[str, Any]) -> dict[str, Any]:
    started = time.perf_counter()
    result = baseline(case["action"])
    return {
        "id": case["id"],
        "expected": case["expected"],
        "native": result,
        "prediction": {**result["risk_flags"], "alignment": result["alignment"]},
        "elapsed_ms": (time.perf_counter() - started) * 1000,
    }


def compare(
    screen: TypedLaya, cases: list[dict[str, Any]], schemas: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    reports: dict[str, list[dict[str, Any]]] = {name: [] for name in schemas}
    native = [native_row(case) for case in cases]
    for index, case in enumerate(cases):
        state = model_state(case["goal"], case["step"], case["action"])
        # Alternate order to reduce a consistent first-schema timing advantage.
        names = list(schemas) if index % 2 == 0 else list(reversed(schemas))
        for name in names:
            print(f"{name}: {case['id']}", flush=True)
            judge = TypedLaya(screen.agent, screen.model_id, schemas[name])
            semantic = judge.assess(state)
            combined = compose(native[index]["native"], semantic, digest(state))
            reports[name].append(
                {
                    "id": case["id"],
                    "expected": case["expected"],
                    "prediction": {**combined["risk_flags"], "alignment": combined["alignment"]},
                    "semantic": semantic,
                    "raw_semantic_alignment": semantic.get("decisions", {})
                    .get("alignment", {})
                    .get("label"),
                    "elapsed_ms": semantic["elapsed_ms"] + native[index]["elapsed_ms"],
                }
            )
    result = {"rules_only": summarize(native)}
    for name, rows in reports.items():
        result[name] = summarize(rows)
        result[name]["raw_semantic_alignment_correct"] = sum(
            row["raw_semantic_alignment"] == row["expected"]["alignment"] for row in rows
        )
    return result


def runtime_probe(screen: TypedLaya, torch: Any, state: dict[str, str]) -> list[dict[str, Any]]:
    results = []
    # Tune only on an existing development example, not the new evaluation labels.
    for threads in (1, 2, 4):
        torch.set_num_threads(threads)
        for name, questions in (
            ("five_questions", QUESTIONS),
            ("alignment_only", {"alignment": QUESTIONS["alignment"]}),
        ):
            judge = TypedLaya(screen.agent, screen.model_id, questions)
            warmup = judge.assess(state)
            samples = [judge.assess(state) for _ in range(2)]
            results.append(
                {
                    "threads": threads,
                    "question_set": name,
                    "warmup": warmup,
                    "samples": samples,
                    "median_ms": statistics.median(x["elapsed_ms"] for x in samples),
                }
            )
            print(
                f"Probe: {threads} threads, {name}: {results[-1]['median_ms']:.1f} ms", flush=True
            )
    return results


def reference_results(
    path: Path, cases: list[dict[str, Any]], schemas: dict[str, dict[str, Any]], model_id: str
) -> dict[str, Any]:
    previous = json.loads(path.read_text(encoding="utf-8"))
    if previous.get("case_digest") != digest(cases) or previous.get("status") != "development_only":
        raise ValueError("Reference cases differ or reference failed")
    by_id = {c["id"]: c for c in cases}
    for name, schema in schemas.items():
        rows = previous["comparisons"][name]["rows"]
        if len(rows) != len(cases) or {r["id"] for r in rows} != set(by_id):
            raise ValueError("Incomplete reference rows")
        for row in rows:
            case = by_id[row["id"]]
            if (
                row["expected"] != case["expected"]
                or row["semantic"]["model_id"] != model_id
                or row["semantic"]["schema_digest"] != digest(schema)
                or row["semantic"]["state_digest"]
                != digest(model_state(case["goal"], case["step"], case["action"]))
            ):
                raise ValueError("Reference state or schema differs")
    return previous["comparisons"]


def quantize_candidate(screen: TypedLaya, torch: Any, encoder_only: bool) -> TypedLaya:
    if encoder_only:
        # The decision head directly reads Linear.weight.dtype. Whole-model
        # dynamic quantization replaces weight with a method and is incompatible.
        model = copy.deepcopy(screen.agent.model)
        model.encoder = torch.ao.quantization.quantize_dynamic(
            model.encoder, {torch.nn.Linear}, dtype=torch.qint8, inplace=True
        )
    else:
        model = torch.ao.quantization.quantize_dynamic(
            screen.agent.model, {torch.nn.Linear}, dtype=torch.qint8, inplace=False
        )
    agent = copy.copy(screen.agent)
    agent.model = model
    return TypedLaya(
        agent, screen.model_id + (":encoder-int8" if encoder_only else ":dynamic-int8")
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-dir", type=Path)
    parser.add_argument("--lock", type=Path, default=Path("training/laya/assets.lock.json"))
    parser.add_argument("--cases", type=Path, default=Path("training/laya/comparison.jsonl"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rules-only", action="store_true")
    parser.add_argument("--runtime-probe", action="store_true")
    parser.add_argument("--dynamic-int8", action="store_true")
    parser.add_argument("--encoder-only", action="store_true")
    parser.add_argument("--reference-report", type=Path)
    args = parser.parse_args()
    report: dict[str, Any] = {"deployment_approved": False, "status": "development_only"}
    try:
        cases = read_cases(args.cases)
        report.update(
            {
                "case_digest": digest(cases),
                "cases": len(cases),
                "label_review": "author-reviewed synthetic; no independent review",
            }
        )
        if args.rules_only:
            report["comparisons"] = {"rules_only": summarize([native_row(c) for c in cases])}
        else:
            if not args.model_dir:
                raise ValueError("Model directory required")
            network_events = []

            def deny_network(event: str, _args: tuple[Any, ...]) -> None:
                if event in {
                    "socket.connect",
                    "socket.getaddrinfo",
                    "socket.sendto",
                    "socket.bind",
                }:
                    network_events.append(event)
                    raise PermissionError("Network disabled in evaluator")

            sys.addaudithook(deny_network)
            print("Loading local CPU model...", flush=True)
            torch = importlib.import_module("torch")
            torch.set_num_threads(4)
            started = time.perf_counter()
            with warnings.catch_warnings(record=True) as captured:
                warnings.simplefilter("always")
                screen = TypedLaya.load(args.model_dir, args.lock)
            report["load_ms"] = (time.perf_counter() - started) * 1000
            report["load_warnings"] = [str(w.message) for w in captured]
            dev = json.loads(Path("training/laya/development.jsonl").read_text().splitlines()[0])
            probes = runtime_probe(screen, torch, dev["state"]) if args.runtime_probe else []
            report["runtime_probes"] = probes
            eligible = [
                p
                for p in probes
                if p["question_set"] == "alignment_only"
                and all(s["status"] == "assessed" for s in p["samples"])
            ]
            threads = min(eligible, key=lambda p: p["median_ms"])["threads"] if eligible else 4
            torch.set_num_threads(threads)
            report["selected_threads"] = threads
            schemas: dict[str, dict[str, Any]] = {
                "rules_plus_original": {"alignment": QUESTIONS["alignment"]},
                "rules_plus_revised": ALIGNMENT_QUESTIONS,
            }
            if args.reference_report:
                report["comparisons"] = reference_results(
                    args.reference_report, cases, schemas, screen.model_id
                )
                report["reference_report_sha256"] = hashlib.sha256(
                    args.reference_report.read_bytes()
                ).hexdigest()
            else:
                report["comparisons"] = compare(screen, cases, schemas)
            # Preserve reference evidence even if a native optimization crashes.
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(
                json.dumps({**report, "status": "candidate_pending"}, indent=2) + "\n",
                encoding="utf-8",
            )
            if args.dynamic_int8:
                try:
                    print("Testing dynamic INT8 candidate...", flush=True)
                    candidate = quantize_candidate(screen, torch, args.encoder_only)
                    candidate_schemas = {"rules_plus_original": schemas["rules_plus_original"]}
                    probe = TypedLaya(
                        candidate.agent, candidate.model_id, schemas["rules_plus_original"]
                    ).assess(dev["state"])
                    if probe["status"] == "assessed":
                        report["dynamic_int8"] = compare(candidate, cases, candidate_schemas)
                    else:
                        report["dynamic_int8"] = {"status": "screen_failed", "probe": probe}
                except Exception as exc:  # noqa: BLE001 - unsupported optimization stays disabled
                    report["dynamic_int8"] = {
                        "status": "unavailable",
                        "error_type": type(exc).__name__,
                    }
                report["quantization_scope"] = "encoder" if args.encoder_only else "whole_model"
            report["peak_process_rss_bytes"] = peak_rss_bytes()
            report["python_network_attempts_blocked"] = network_events
        return_code = 0
    except Exception as exc:  # noqa: BLE001 - failed runs still leave reviewable evidence
        report.update({"status": "failed", "error_type": type(exc).__name__})
        return_code = 2
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(f"Report: {args.output}", flush=True)
    return return_code


if __name__ == "__main__":
    raise SystemExit(main())
