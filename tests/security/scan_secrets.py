"""Fails if the repository contains anything that looks like a committed secret."""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SKIP_DIRS = {".git", "node_modules", "dist", "target", "models", "__pycache__", ".venv"}
SKIP_SUFFIXES = {".gguf", ".jsonl", ".png", ".ico", ".icns", ".lock"}
PATTERNS = {
    "AWS access key": re.compile(r"AKIA[0-9A-Z]{16}"),
    "private key": re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH |)PRIVATE KEY-----"),
    "GitHub token": re.compile(r"gh[pousr]_[A-Za-z0-9]{36}"),
    "Hugging Face token": re.compile(r"hf_[A-Za-z0-9]{34}"),
}


def iter_files(root: Path):
    for path in root.rglob("*"):
        if any(part in SKIP_DIRS for part in path.relative_to(root).parts):
            continue
        if path.is_file() and path.suffix not in SKIP_SUFFIXES:
            yield path


def scan(root: Path) -> list[str]:
    findings = []
    for path in iter_files(root):
        try:
            content = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for name, pattern in PATTERNS.items():
            if pattern.search(content):
                findings.append(f"{path.relative_to(root)}: possible {name}")
    return findings


def main() -> int:
    findings = scan(REPO_ROOT)
    for finding in findings:
        print(f"[secret-scan] {finding}")
    print(f"[secret-scan] {len(findings)} finding(s)")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
