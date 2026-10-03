import os
import sys

# Add the src folder to path so we can import laya and model_loader
sys.path.append(os.path.join(os.path.dirname(__file__), "..", "services", "mlhost"))
sys.path.append(os.path.join(os.path.dirname(__file__), "..", "services", "mlhost", "src"))

from laya import decide

print("Testing Laya offline inference...")

test_cases = [
    {"filename": "windows_update.dll", "av_hits": 0, "signed": True},
    {"filename": "free_minecraft_hack.exe", "av_hits": 12, "signed": False},
    {"filename": "unknown_script.bat", "av_hits": 2, "signed": False}
]

for i, data in enumerate(test_cases):
    print(f"\n--- Test Case {i+1} ---")
    print(f"Input: {data}")
    decision = decide(i, data)
    print(f"Laya's Decision: {decision}")
