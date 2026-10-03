import os
import sys
import re

def check_for_leaked_secrets(root_dir):
    print("[Security Smoke Test] Scanning codebase for leaked keys/secrets...")
    
    # Very basic scanner looking for things that look like hardcoded AWS keys or generic secrets
    patterns = [
        re.compile(r"AKIA[0-9A-Z]{16}"), # AWS Key
        re.compile(r"-----BEGIN RSA PRIVATE KEY-----"), # Private keys
    ]
    
    issues_found = 0
    skip_dirs = ['node_modules', '.git', 'dist', 'target', 'models', '__pycache__']
    
    for dirpath, dirnames, filenames in os.walk(root_dir):
        # Exclude directories
        dirnames[:] = [d for d in dirnames if d not in skip_dirs]
        
        for filename in filenames:
            # Skip massive files
            if filename.endswith(".gguf") or filename.endswith(".jsonl"):
                continue
                
            filepath = os.path.join(dirpath, filename)
            try:
                with open(filepath, 'r', encoding='utf-8') as f:
                    content = f.read()
                    for pattern in patterns:
                        if pattern.search(content):
                            print(f"[ERROR] Potential leaked secret found in: {filepath}")
                            issues_found += 1
            except Exception:
                pass # Skip binary files that fail utf-8 decoding
                
    if issues_found > 0:
        print(f"[Security Smoke Test] FAILED! Found {issues_found} potential secrets.")
        sys.exit(1)
    else:
        print("[Security Smoke Test] SUCCESS! No obvious hardcoded secrets found.")

if __name__ == "__main__":
    check_for_leaked_secrets(os.path.join(os.path.dirname(__file__), "..", ".."))
