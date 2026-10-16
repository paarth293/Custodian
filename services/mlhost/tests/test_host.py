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


def test_planted_approve_or_delete_is_refused(monkeypatch) -> None:
    from mlhost import laya

    class FakeModel:
        def __init__(self, action: str):
            self.action = action

        def create_chat_completion(self, **kwargs):
            import json

            return {"choices": [{"message": {"content": json.dumps({"action": self.action})}}]}

    # Model attempts to approve -> refused, abstains
    monkeypatch.setattr("mlhost.laya.load_model", lambda: FakeModel("approve"))
    assert laya.decide({"filename": "malware.exe", "av_hits": 0}) == "abstained"

    # Model attempts to delete directly -> refused, abstains
    monkeypatch.setattr("mlhost.laya.load_model", lambda: FakeModel("delete"))
    assert laya.decide({"filename": "malware.exe", "av_hits": 0}) == "abstained"


def test_av_hits_deterministically_forces_quarantine_ignoring_model(monkeypatch) -> None:
    from mlhost import laya

    class FakeModel:
        def create_chat_completion(self, **kwargs):
            import json

            return {"choices": [{"message": {"content": json.dumps({"action": "approve"})}}]}

    monkeypatch.setattr("mlhost.laya.load_model", FakeModel)
    # AV hits > 0 must quarantine even if ML claimed "approve" (monotonicity / F45)
    assert laya.decide({"filename": "virus.exe", "av_hits": 3}) == "quarantine"


def test_valid_escalate_quarantine_and_review(monkeypatch) -> None:
    from mlhost import laya

    class FakeModel:
        def __init__(self, action: str):
            self.action = action

        def create_chat_completion(self, **kwargs):
            import json

            return {"choices": [{"message": {"content": json.dumps({"action": self.action})}}]}

    monkeypatch.setattr("mlhost.laya.load_model", lambda: FakeModel("quarantine"))
    assert laya.decide({"filename": "suspicious.exe", "av_hits": 0}) == "quarantine"

    monkeypatch.setattr("mlhost.laya.load_model", lambda: FakeModel("review"))
    assert laya.decide({"filename": "suspicious.exe", "av_hits": 0}) == "review"


def test_malformed_model_action_abstains(monkeypatch) -> None:
    from mlhost import laya

    class FakeModel:
        def create_chat_completion(self, **kwargs):
            return {"choices": [{"message": {"content": '{"action": ["approve"]}'}}]}

    monkeypatch.setattr("mlhost.laya.load_model", FakeModel)
    assert laya.decide({"filename": "file.exe", "av_hits": 0}) == "abstained"
