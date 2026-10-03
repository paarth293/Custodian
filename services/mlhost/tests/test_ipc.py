import io

import pytest

from mlhost.ipc import FrameError, read_frame, read_frame_from, write_frame

KEY = b"test-key"


def test_round_trip() -> None:
    assert read_frame(write_frame({"a": 1}, KEY), KEY) == {"a": 1}


def test_rejects_wrong_key() -> None:
    with pytest.raises(FrameError, match="signature"):
        read_frame(write_frame({"a": 1}, KEY), b"other")


def test_rejects_bad_magic() -> None:
    frame = b"XUST" + write_frame({"a": 1}, KEY)[4:]
    with pytest.raises(FrameError, match="magic"):
        read_frame(frame, KEY)


def test_stream_reader_returns_none_at_eof() -> None:
    stream = io.BytesIO(write_frame({"a": 1}, KEY))
    assert read_frame_from(stream, KEY) == {"a": 1}
    assert read_frame_from(stream, KEY) is None


def test_matches_rust_wire_format() -> None:
    frame = write_frame({"x": 1}, KEY)
    assert frame[:4] == b"CUST"
    assert int.from_bytes(frame[4:8], "little") == len(b'{"x":1}')
