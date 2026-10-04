"""Request loop: reads framed requests on stdin and writes framed responses."""

from __future__ import annotations

import os
import sys
from typing import Any, BinaryIO

from mlhost import laya
from mlhost.ipc import FrameError, StreamDesyncError, read_frame_from, write_frame

EMBEDDING_DIM = 1536
KEY_ENV_VAR = "CUSTODIAN_IPC_KEY"


def handle(request: dict[str, Any]) -> dict[str, Any]:
    action = request.get("action")
    if action == "ping":
        return {"status": "ok"}
    if action == "extract_embeddings":
        # Placeholder: zero vector until the embedding model is wired in.
        return {"status": "ok", "embedding": [0.0] * EMBEDDING_DIM}
    if action == "decide":
        data = request.get("data")
        if not isinstance(data, dict):
            return {"status": "error", "error": "decide requires a 'data' object"}
        return {"status": "ok", "decision": laya.decide(data)}
    return {"status": "error", "error": f"unknown action: {action!r}"}


def safe_handle(request: dict[str, Any]) -> dict[str, Any]:
    """Like :func:`handle`, but turns any failure into an error response."""
    try:
        return handle(request)
    except Exception as exc:  # noqa: BLE001 - one bad request must not stop the host
        print(f"[mlhost] request failed: {exc!r}", file=sys.stderr)
        return {"status": "error", "error": "internal error"}


def process_frames(stdin: BinaryIO, stdout: BinaryIO, key: bytes) -> None:
    """Serves requests until ``stdin`` is exhausted or desynchronised.

    Complete frames that fail validation are dropped without a response so an
    unauthenticated peer learns nothing. A header that cannot be trusted ends
    the session, because the next frame boundary is unknown.
    """
    while True:
        try:
            request = read_frame_from(stdin, key)
        except StreamDesyncError as exc:
            print(f"[mlhost] closing stream: {exc}", file=sys.stderr)
            return
        except FrameError as exc:
            print(f"[mlhost] dropped frame: {exc}", file=sys.stderr)
            continue
        if request is None:
            return
        stdout.write(write_frame(safe_handle(request), key))
        stdout.flush()


def main() -> int:
    key = os.environ.get(KEY_ENV_VAR)
    if not key:
        print(f"[mlhost] {KEY_ENV_VAR} is not set; refusing to start.", file=sys.stderr)
        return 2
    process_frames(sys.stdin.buffer, sys.stdout.buffer, key.encode("utf-8"))
    return 0
