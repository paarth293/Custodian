"""Safety classifier. Placeholder: always escalates to the user."""

from typing import Any


def check_safety(data: Any) -> str:
    del data
    return "escalate"
