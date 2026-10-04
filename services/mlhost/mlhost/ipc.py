"""Authenticated IPC framing, byte-compatible with the `custodian-ipc` crate.

Wire format: ``b"CUST" | len: u32 LE | HMAC-SHA256(payload) | payload``.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import struct
from typing import Any, BinaryIO

MAGIC = b"CUST"
HEADER_SIZE = 4 + 4 + 32
MAX_FRAME_SIZE = 10 * 1024 * 1024


class FrameError(ValueError):
    """A complete frame was read but is malformed or unauthenticated.

    The stream is still aligned on a frame boundary, so reading may continue.
    """


class StreamDesyncError(FrameError):
    """The frame header is unusable (bad magic or oversized length).

    The body length cannot be trusted, so the stream must be closed.
    """


def _sign(key: bytes, payload: bytes) -> bytes:
    return hmac.new(key, payload, hashlib.sha256).digest()


def write_frame(payload: dict[str, Any], key: bytes) -> bytes:
    body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    if len(body) > MAX_FRAME_SIZE:
        raise FrameError("frame too large")
    return MAGIC + struct.pack("<I", len(body)) + _sign(key, body) + body


def read_frame(data: bytes, key: bytes) -> dict[str, Any]:
    if len(data) < HEADER_SIZE:
        raise FrameError("buffer too small")
    if data[:4] != MAGIC:
        raise FrameError("invalid magic")

    (length,) = struct.unpack("<I", data[4:8])
    if length > MAX_FRAME_SIZE:
        raise FrameError("frame too large")
    if len(data) < HEADER_SIZE + length:
        raise FrameError("incomplete frame")

    signature = data[8:HEADER_SIZE]
    body = data[HEADER_SIZE : HEADER_SIZE + length]
    if not hmac.compare_digest(_sign(key, body), signature):
        raise FrameError("invalid signature")

    try:
        payload = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise FrameError(f"invalid JSON payload: {exc}") from exc
    if not isinstance(payload, dict):
        raise FrameError("payload must be a JSON object")
    return payload


def read_frame_from(stream: BinaryIO, key: bytes) -> dict[str, Any] | None:
    """Reads one frame from ``stream``; returns ``None`` at end of stream.

    Raises :class:`StreamDesyncError` when the header cannot be trusted and
    :class:`FrameError` when a complete frame fails validation.
    """
    header = stream.read(HEADER_SIZE)
    if len(header) < HEADER_SIZE:
        return None
    if header[:4] != MAGIC:
        raise StreamDesyncError("invalid magic")
    (length,) = struct.unpack("<I", header[4:8])
    if length > MAX_FRAME_SIZE:
        raise StreamDesyncError("frame too large")
    body = stream.read(length)
    if len(body) < length:
        return None
    return read_frame(header + body, key)
