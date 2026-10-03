from pathlib import Path

FIXTURE_MARKER = ".custodian-fixture-root"


class FixtureRoot:
    """A directory proven to be a generated fixture tree (see custodian-fixtures)."""

    def __init__(self, path: str | Path) -> None:
        self._path = Path(path)
        if not (self._path / FIXTURE_MARKER).exists():
            raise RuntimeError(f"SECURITY: test attempted to use a non-fixture path: {self._path}")

    @property
    def path(self) -> Path:
        return self._path
