import io
import os
import sys

import pytest

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from custodian_ipc import read_frame, write_frame
from main import process_frames

KEY = b"super-secret-key"


def run_loop(monkeypatch: pytest.MonkeyPatch, data: bytes) -> bytes:
    out = io.TextIOWrapper(io.BytesIO())
    monkeypatch.setattr(sys, "stdin", io.TextIOWrapper(io.BytesIO(data)))
    monkeypatch.setattr(sys, "stdout", out)
    process_frames()
    out.flush()
    return out.buffer.getvalue()


def test_multiple_frames(monkeypatch: pytest.MonkeyPatch) -> None:
    f1 = write_frame({"action": "extract_embeddings"}, KEY)
    f2 = write_frame({"action": "ping"}, KEY)

    output = run_loop(monkeypatch, f1 + f2)

    # Only the extract_embeddings frame gets a reply, and it must verify.
    reply = read_frame(output, KEY)
    assert reply["status"] == "ok"
    assert len(reply["embedding"]) == 1536
    assert len(output) == 40 + len(
        write_frame({"status": "ok", "embedding": [0.0] * 1536}, KEY)[40:]
    )


def test_bad_signature_is_dropped(monkeypatch: pytest.MonkeyPatch) -> None:
    frame = bytearray(write_frame({"action": "extract_embeddings"}, KEY))
    frame[10] ^= 0xFF  # corrupt the signature

    assert run_loop(monkeypatch, bytes(frame)) == b""
