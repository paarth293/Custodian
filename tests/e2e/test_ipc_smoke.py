"""End-to-end smoke test: spawn the ML host and exchange authenticated frames."""

import io
import os
import subprocess
import sys
from pathlib import Path

from mlhost.ipc import read_frame_from, write_frame

MLHOST_DIR = Path(__file__).resolve().parents[2] / "services" / "mlhost"
KEY = "e2e-test-key"


def test_mlhost_answers_over_stdio(tmp_path: Path) -> None:
    env = {
        **os.environ,
        "CUSTODIAN_IPC_KEY": KEY,
        "CUSTODIAN_MODELS_DIR": str(tmp_path),
        "PYTHONPATH": str(MLHOST_DIR),
    }
    requests = [
        {"action": "ping"},
        {"action": "decide", "data": {"filename": "e2e.exe", "av_hits": 15, "signed": False}},
    ]
    stdin = b"".join(write_frame(r, KEY.encode()) for r in requests)

    proc = subprocess.run(
        [sys.executable, "-m", "mlhost"],
        input=stdin,
        capture_output=True,
        env=env,
        timeout=30,
        check=True,
    )

    stdout = io.BytesIO(proc.stdout)
    assert read_frame_from(stdout, KEY.encode()) == {"status": "ok"}
    decision = read_frame_from(stdout, KEY.encode())
    assert decision is not None
    assert decision["status"] == "ok"
    assert decision["decision"] in {"approve", "delete", "quarantine", "abstained"}


def test_mlhost_refuses_to_start_without_key() -> None:
    env = {k: v for k, v in os.environ.items() if k != "CUSTODIAN_IPC_KEY"}
    env["PYTHONPATH"] = str(MLHOST_DIR)
    proc = subprocess.run(
        [sys.executable, "-m", "mlhost"], input=b"", capture_output=True, env=env, timeout=30
    )
    assert proc.returncode == 2
