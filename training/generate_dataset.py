import json
import random

print("Generating training dataset for Laya...")

examples = []

# Helper function to format the data exactly how Gemma expects it
def add_example(filename, av_hits, signed, expected_action, reason):
    user_prompt = f"Analyze file for security decision:\nFilename: {filename}\nAV Hits: {av_hits}\nSigned: {signed}"
    
    # Laya must always respond in strict JSON
    assistant_response = json.dumps({"action": expected_action, "reason": reason})
    
    examples.append({
        "messages": [
            {"role": "user", "content": user_prompt},
            {"role": "model", "content": assistant_response}
        ]
    })

# --- Let's generate 90 synthetic examples ---

# 1. SAFE FILES (0 hits, signed) -> approve
for i in range(30):
    add_example(f"windows_update_{i}.dll", 0, True, "approve", "Trusted signature and clean scan.")

# 2. OBVIOUS MALWARE (5+ hits, unsigned) -> delete
for i in range(30):
    add_example(f"free_minecraft_hack_{i}.exe", random.randint(5, 15), False, "delete", "Multiple antivirus engines detected malware.")

# 3. SUSPICIOUS FILES (1-4 hits, or unsigned) -> quarantine
for i in range(30):
    add_example(f"unknown_script_{i}.bat", random.randint(1, 4), False, "quarantine", "Suspicious heuristics detected, isolating for manual review.")

# Save to a JSONL file
output_path = "training/dataset.jsonl"
with open(output_path, "w") as f:
    for ex in examples:
        f.write(json.dumps(ex) + "\n")

print(f"Success! Generated {len(examples)} examples in {output_path}")