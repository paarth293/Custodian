"""Synthetic development evaluation; never authorizes production deployment."""

from __future__ import annotations

import argparse
import hashlib
import importlib
import importlib.metadata
import json
import platform
import statistics
import sys
import time
import warnings
from collections.abc import Callable
from pathlib import Path
from typing import Any

from mlhost.typed_decisions import ALIGNMENTS, RISK_IDS, TypedLaya, validate_state


def load_cases(path: Path) -> list[dict[str, Any]]:
    cases = [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()
    ]
    if not cases:
        raise ValueError("Empty evaluation set")
    seen = set()
    for case in cases:
        if not isinstance(case.get("id"), str) or case["id"] in seen:
            raise ValueError("Missing or duplicate case ID")
        seen.add(case["id"])
        validate_state(case["state"])
        expected = case["expected"]
        if set(expected) != {*RISK_IDS, "alignment"}:
            raise ValueError("Every decision needs a reference label")
        if any(type(expected[k]) is not bool for k in RISK_IDS):
            raise ValueError("Risk reference labels must be boolean")
        if expected["alignment"] not in ALIGNMENTS:
            raise ValueError("Unknown reference alignment")
    return cases


def evaluate(
    screen: TypedLaya,
    cases: list[dict[str, Any]],
    progress: Callable[[str], None] | None = None,
) -> dict[str, Any]:
    rows = []
    correct = dict.fromkeys([*RISK_IDS, "alignment"], 0)
    positives = dict.fromkeys(RISK_IDS, 0)
    misses = dict.fromkeys(RISK_IDS, 0)
    abstained = 0
    for case in cases:
        if progress:
            progress(case["id"])
        result = screen.assess(case["state"])
        row = {"id": case["id"], "result": result, "expected": case["expected"]}
        abstained += result["status"] != "assessed"
        for key in RISK_IDS:
            positives[key] += case["expected"][key]
        if result["status"] == "assessed":
            # 0.5 is a diagnostic cutoff, not a calibrated deployment threshold.
            predicted = {k: result["decisions"][k]["probability"] >= 0.5 for k in RISK_IDS}
            predicted["alignment"] = result["decisions"]["alignment"]["label"]
            row["predicted"] = predicted
            for key in correct:
                correct[key] += predicted[key] == case["expected"][key]
            for key in RISK_IDS:
                misses[key] += case["expected"][key] and not predicted[key]
        else:
            # Unscored cases remain in the accuracy denominator, never disappear.
            for key in RISK_IDS:
                misses[key] += case["expected"][key]
        rows.append(row)
    latencies = sorted(r["result"]["elapsed_ms"] for r in rows)
    return {
        "status": "development_evaluation_only",
        "deployment_approved": False,
        "cases": len(cases),
        "abstentions": abstained,
        "accuracy_including_abstentions": {k: v / len(cases) for k, v in correct.items()},
        "critical_positive_counts": positives,
        "critical_misses_including_abstentions": misses,
        "latency_ms": {
            "p50": statistics.median(latencies),
            "p95": latencies[max(0, math_ceil_95(len(latencies)) - 1)],
        },
        "threshold_note": "0.5 diagnostic only; no domain calibration or activation gate",
        "rows": rows,
    }


def math_ceil_95(count: int) -> int:
    return (95 * count + 99) // 100


def peak_rss_bytes() -> int:
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes

        class Counters(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
                (name, ctypes.c_size_t)
                for name in (
                    "PeakWorkingSetSize",
                    "WorkingSetSize",
                    "QuotaPeakPagedPoolUsage",
                    "QuotaPagedPoolUsage",
                    "QuotaPeakNonPagedPoolUsage",
                    "QuotaNonPagedPoolUsage",
                    "PagefileUsage",
                    "PeakPagefileUsage",
                )
            ]

        counters = Counters()
        counters.cb = ctypes.sizeof(counters)
        process = ctypes.windll.kernel32.GetCurrentProcess
        process.restype = wintypes.HANDLE
        query = ctypes.windll.psapi.GetProcessMemoryInfo
        query.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
        if not query(process(), ctypes.byref(counters), counters.cb):
            raise OSError("Cannot measure peak process memory")
        return int(counters.PeakWorkingSetSize)
    import resource

    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return int(peak if sys.platform == "darwin" else peak * 1024)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--lock", type=Path, default=Path("training/laya/assets.lock.json"))
    parser.add_argument("--cases", type=Path, default=Path("training/laya/development.jsonl"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument(
        "--max-cases", type=int, help="Explicit diagnostic subset, not full evaluation"
    )
    args = parser.parse_args()
    try:
        initialization_started = time.perf_counter()
        network_attempts: list[str] = []

        def deny_network(event: str, _args: tuple[Any, ...]) -> None:
            if event in {"socket.connect", "socket.getaddrinfo", "socket.sendto", "socket.bind"}:
                network_attempts.append(event)
                raise PermissionError("Network is disabled in the development evaluator")

        # Additional Python-level diagnostic guard, not an OS sandbox boundary.
        sys.addaudithook(deny_network)
        cases = load_cases(args.cases)
        source_case_count = len(cases)
        if args.max_cases is not None:
            if args.max_cases < 1:
                raise ValueError("Case limit must be positive")
            cases = cases[: args.max_cases]
        if not 1 <= args.threads <= 16:
            raise ValueError("Threads must be between 1 and 16")
        print("Initializing CPU runtime...", flush=True)
        importlib.import_module("torch").set_num_threads(args.threads)
        started = time.perf_counter()
        print("Verifying and loading pinned local model...", flush=True)
        with warnings.catch_warnings(record=True) as captured:
            warnings.simplefilter("always")
            screen = TypedLaya.load(args.model_dir, args.lock)
        load_ms = (time.perf_counter() - started) * 1000
        initialization_ms = (time.perf_counter() - initialization_started) * 1000
        # Separate the cold first forward from measured warm-case timings.
        print("Running cold forward and synthetic development cases...", flush=True)
        warmup = screen.assess(cases[0]["state"])
        report = evaluate(
            screen, cases, lambda case_id: print(f"Assessing {case_id}...", flush=True)
        )
        report.update(
            {
                "load_ms": load_ms,
                "source_case_count": source_case_count,
                "initialization_including_torch_ms": initialization_ms,
                "cpu_threads": args.threads,
                "peak_process_rss_bytes": peak_rss_bytes(),
                "load_warnings": [str(w.message) for w in captured],
                "python_network_attempts_blocked": network_attempts,
                "warmup": warmup,
                "case_file_sha256": hashlib.sha256(args.cases.read_bytes()).hexdigest(),
                "platform": platform.platform(),
                "python": platform.python_version(),
                "versions": {
                    k: importlib.metadata.version(k)
                    for k in ("laya", "torch", "transformers", "huggingface_hub", "safetensors")
                },
                "isolation": "development process; not an OS sandbox",
            }
        )
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(report, indent=2, allow_nan=False) + "\n", encoding="utf-8"
        )
        print(
            json.dumps({k: v for k, v in report.items() if k not in {"rows", "warmup"}}, indent=2)
        )
        return 2 if report["abstentions"] else 0
    except Exception as exc:  # noqa: BLE001 - command failures must be explicit
        print(f"Laya validation failed: {type(exc).__name__}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
