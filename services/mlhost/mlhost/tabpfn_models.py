"""TabPFN-based predictors. Placeholders with conservative outputs."""

from typing import Any


def predict_regret(context_rows: Any, new_row: Any) -> str:
    del context_rows, new_row
    return "escalate"


def predict_forecast(time_series: Any) -> str:
    del time_series
    return "2027-01-01"


def detect_anomaly(plan: Any) -> bool:
    del plan
    return False
