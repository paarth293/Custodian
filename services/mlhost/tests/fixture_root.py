from pathlib import Path


class FixtureRoot:
    def __init__(self, path: str | Path):
        self._path = Path(path)
        marker = self._path / ".custodian-fixture-root"
        if not marker.exists():
            raise RuntimeError(f"SECURITY: Test attempted to use a non-fixture path. Path: {self._path}")
            
    @property
    def path(self) -> Path:
        return self._path
