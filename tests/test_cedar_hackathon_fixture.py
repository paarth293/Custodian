"""Tests for the isolated synthetic Cedar hackathon fixture."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "create-cedar-hackathon-fixture.py"


def test_fixture_writes_expected_tree_and_manifest(tmp_path: Path) -> None:
    output = tmp_path / "fixture"
    result = subprocess.run(
        [sys.executable, str(SCRIPT), str(output)], capture_output=True, text=True, check=True
    )

    manifest_path = output / "manifest.json"
    assert result.stdout.strip() == str(manifest_path)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert len(manifest["files"]) == 6
    assert not (output / "private" / "never-observe-canary.txt").read_text(encoding="utf-8") == ""
    for entry in manifest["files"]:
        data = (output / entry["path"]).read_bytes()
        assert hashlib.sha256(data).hexdigest() == entry["sha256"]
    assert manifest["expected_assertions"]["public_file_count"] == 5


def test_fixture_does_not_write_outside_output_root(tmp_path: Path) -> None:
    output = tmp_path / "fixture"
    subprocess.run([sys.executable, str(SCRIPT), str(output)], check=True)
    assert not (tmp_path / "private" / "never-observe-canary.txt").exists()
