"""Legacy GGUF-adviser dataset generator (not the official typed Laya workflow).

DEPRECATED for new training: approve/delete targets conflict with the current
advisory interface. See training/laya/README.md and the typed decision rubrics.

Uses the same prompt builder as inference so the two cannot drift apart:
    PYTHONPATH=services/mlhost python training/generate_dataset.py
"""

import json
import random
from pathlib import Path

from mlhost.laya import build_prompt

OUTPUT = Path(__file__).resolve().parent / "dataset.jsonl"
EXAMPLES_PER_CLASS = 30
SEED = 1337


def example(filename: str, av_hits: int, signed: bool, action: str, reason: str) -> dict:
    return {
        "messages": [
            {"role": "user", "content": build_prompt(filename, av_hits, signed)},
            {"role": "model", "content": json.dumps({"action": action, "reason": reason})},
        ]
    }


def main() -> None:
    rng = random.Random(SEED)
    examples = []
    for i in range(EXAMPLES_PER_CLASS):
        examples.append(
            example(
                f"windows_update_{i}.dll", 0, True, "approve", "Trusted signature and clean scan."
            )
        )
    for i in range(EXAMPLES_PER_CLASS):
        examples.append(
            example(
                f"free_minecraft_hack_{i}.exe",
                rng.randint(5, 15),
                False,
                "delete",
                "Multiple antivirus engines detected malware.",
            )
        )
    for i in range(EXAMPLES_PER_CLASS):
        examples.append(
            example(
                f"unknown_script_{i}.bat",
                rng.randint(1, 4),
                False,
                "quarantine",
                "Suspicious heuristics detected, isolating for manual review.",
            )
        )

    with OUTPUT.open("w", encoding="utf-8") as f:
        for ex in examples:
            f.write(json.dumps(ex) + "\n")
    print(f"Generated {len(examples)} examples in {OUTPUT}")


if __name__ == "__main__":
    main()
