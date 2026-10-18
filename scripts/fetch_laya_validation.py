"""Development-only download of the exact assets in the typed Laya lock."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import urllib.request
from pathlib import Path


def fetch(url: str, destination: Path, expected_sha: str, expected_size: int | None) -> None:
    if destination.is_file():
        with destination.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() == expected_sha:
                print(f"Verified existing {destination.name}", flush=True)
                return
        raise ValueError("Existing asset differs; use a fresh output directory")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_name(destination.name + ".partial")
    # Do not delete an incomplete file from an earlier run or another process.
    out = temporary.open("xb")
    sha = hashlib.sha256()
    count = 0
    print(f"Fetching pinned {destination.name}", flush=True)
    try:
        with out, urllib.request.urlopen(url, timeout=120) as source:
            while chunk := source.read(1024 * 1024):
                count += len(chunk)
                if count > (expected_size if expected_size is not None else 64 * 1024 * 1024):
                    raise ValueError("Download exceeded pinned size or source archive budget")
                sha.update(chunk)
                out.write(chunk)
            out.flush()
            os.fsync(out.fileno())
        if (
            expected_size is not None and count != expected_size
        ) or sha.hexdigest() != expected_sha:
            raise ValueError("Download integrity mismatch")
        temporary.replace(destination)
    finally:
        out.close()
        temporary.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", type=Path, default=Path("training/laya/assets.lock.json"))
    parser.add_argument("--output", type=Path, default=Path("target/laya-validation/model"))
    parser.add_argument(
        "--sdk-archive", type=Path, help="Also fetch the pinned official SDK source"
    )
    args = parser.parse_args()
    lock = json.loads(args.lock.read_text(encoding="utf-8"))
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=True)
    for relative, expected in lock["files"].items():
        destination = root / relative
        if not destination.resolve().is_relative_to(root):
            raise ValueError("Asset escaped output root")
        url = f"https://huggingface.co/{lock['repository']}/resolve/{lock['revision']}/{relative}"
        fetch(url, destination, expected["sha256"], expected["bytes"])
    if args.sdk_archive:
        fetch(
            "https://codeload.github.com/NandhaKishorM/laya/zip/" + lock["sdk_revision"],
            args.sdk_archive.resolve(),
            lock["sdk_archive_sha256"],
            None,
        )
    print("All local model assets match the pinned lock.", flush=True)


if __name__ == "__main__":
    main()
