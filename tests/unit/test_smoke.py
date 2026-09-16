"""T-001 — smoke test: the package imports and reports its development version."""

import ferrite


def test_imports() -> None:
    assert ferrite.__version__ == "0.1.0.dev0"
