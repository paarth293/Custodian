"""Manual check of Laya's offline inference against a few sample files.

Requires the model in models/ and requirements-model.txt installed:
    PYTHONPATH=services/mlhost python scripts/test_laya.py
"""

from mlhost.laya import decide

SAMPLES = [
    {"filename": "windows_update.dll", "av_hits": 0, "signed": True},
    {"filename": "free_minecraft_hack.exe", "av_hits": 12, "signed": False},
    {"filename": "unknown_script.bat", "av_hits": 2, "signed": False},
]

for sample in SAMPLES:
    print(f"{sample} -> {decide(sample)}")
