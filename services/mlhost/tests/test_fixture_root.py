from pathlib import Path

import pytest

from fixture_root import FIXTURE_MARKER, FixtureRoot


def test_accepts_marked_directory(tmp_path: Path) -> None:
    (tmp_path / FIXTURE_MARKER).touch()
    assert FixtureRoot(tmp_path).path == tmp_path


def test_rejects_unmarked_directory(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="SECURITY"):
        FixtureRoot(tmp_path)
