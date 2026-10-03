import hashlib
import hmac
import io

from mlhost.host import EMBEDDING_DIM, process_frames
from mlhost.ipc import MAX_FRAME_SIZE, read_frame_from, write_frame

KEY = b"test-key"


def hmac_sha256(key: bytes, body: bytes) -> bytes:
    return hmac.new(key, body, hashlib.sha256).digest()


def run(*requests: dict, key: bytes = KEY) -> list[dict]:
    stdin = io.BytesIO(b"".join(write_frame(r, key) for r in requests))
    stdout = io.BytesIO()
    process_frames(stdin, stdout, KEY)

    stdout.seek(0)
    responses = []
    while (frame := read_frame_from(stdout, KEY)) is not None:
        responses.append(frame)
    return responses


def test_serves_multiple_frames() -> None:
    embed, ping = run({"action": "extract_embeddings"}, {"action": "ping"})
    assert embed["status"] == "ok"
    assert len(embed["embedding"]) == EMBEDDING_DIM
    assert ping == {"status": "ok"}


def test_decide_abstains_without_model(tmp_path, monkeypatch) -> None:
    monkeypatch.setenv("CUSTODIAN_MODELS_DIR", str(tmp_path))
    (response,) = run({"action": "decide", "data": {"filename": "x.exe"}})
    assert response == {"status": "ok", "decision": "abstained"}


def test_unknown_action_is_an_error() -> None:
    (response,) = run({"action": "rm -rf"})
    assert response["status"] == "error"


def test_drops_unauthenticated_frames() -> None:
    assert run({"action": "ping"}, key=b"attacker") == []


def test_bad_decide_input_abstains() -> None:
    (response,) = run({"action": "decide", "data": {"av_hits": "many"}})
    assert response == {"status": "ok", "decision": "abstained"}


def test_handler_failure_returns_error_and_keeps_serving(monkeypatch) -> None:
    def boom(_data: dict) -> str:
        raise RuntimeError("boom")

    monkeypatch.setattr("mlhost.host.laya.decide", boom)
    failed, ping = run({"action": "decide", "data": {}}, {"action": "ping"})
    assert failed == {"status": "error", "error": "internal error"}
    assert ping == {"status": "ok"}


def test_oversized_header_closes_stream() -> None:
    oversized = b"CUST" + (MAX_FRAME_SIZE + 1).to_bytes(4, "little") + b"\0" * 32
    stdin = io.BytesIO(oversized + b"x" * 100 + write_frame({"action": "ping"}, KEY))
    stdout = io.BytesIO()
    process_frames(stdin, stdout, KEY)
    assert stdout.getvalue() == b""


def test_invalid_json_is_dropped_and_stream_continues() -> None:
    body = b"not json"
    bad = b"CUST" + len(body).to_bytes(4, "little") + hmac_sha256(KEY, body) + body
    stdin = io.BytesIO(bad + write_frame({"action": "ping"}, KEY))
    stdout = io.BytesIO()
    process_frames(stdin, stdout, KEY)
    stdout.seek(0)
    assert read_frame_from(stdout, KEY) == {"status": "ok"}
