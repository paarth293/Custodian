"""Generates the synthetic fine-tuning dataset for Laya (training/dataset.jsonl).

The prompt format must match ``mlhost.laya.build_prompt``.
"""

import json
import random
from pathlib import Path

OUTPUT = Path(__file__).resolve().parent / "dataset.jsonl"
EXAMPLES_PER_CLASS = 30
SEED = 1337


def prompt(filename: str, av_hits: int, signed: bool) -> str:
    return (
        "Analyze file for security decision:\n"
        f"Filename: {filename}\n"
        f"AV Hits: {av_hits}\n"
        f"Signed: {signed}"
    )


def example(filename: str, av_hits: int, signed: bool, action: str, reason: str) -> dict:
    return {
        "messages": [
            {"role": "user", "content": prompt(filename, av_hits, signed)},
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
