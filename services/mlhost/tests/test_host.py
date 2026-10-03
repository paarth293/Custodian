import io

from mlhost.host import EMBEDDING_DIM, process_frames
from mlhost.ipc import read_frame_from, write_frame

KEY = b"test-key"


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
