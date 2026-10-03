import subprocess
import json
import struct
import hmac
import hashlib
import sys

def write_frame(payload, key):
    payload_bytes = json.dumps(payload).encode('utf-8')
    magic = b"Laya"
    length = struct.pack("<I", len(payload_bytes))
    sig = hmac.new(key, payload_bytes, hashlib.sha256).digest()
    return magic + length + sig + payload_bytes

def main():
    print("[E2E Smoke Test] Starting Laya IPC Host...")
    key = b"super-secret-key"
    
    # Start mlhost
    process = subprocess.Popen(
        [sys.executable, "services/mlhost/main.py"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    
    # Craft a decide request frame
    payload = {
        "action": "decide",
        "d_num": 1,
        "data": {
            "filename": "e2e_test_malware.exe",
            "av_hits": 15,
            "signed": False
        }
    }
    
    print(f"[E2E Smoke Test] Sending payload: {payload}")
    frame = write_frame(payload, key)
    process.stdin.write(frame)
    process.stdin.flush()
    
    # Wait for response frame
    magic = process.stdout.read(4)
    if not magic:
        print("[E2E Smoke Test] Error: No response from host!")
        process.kill()
        sys.exit(1)
        
    length_bytes = process.stdout.read(4)
    length = struct.unpack("<I", length_bytes)[0]
    sig = process.stdout.read(32) # Ignore sig validation for quick smoke test
    response_bytes = process.stdout.read(length)
    
    response = json.loads(response_bytes.decode('utf-8'))
    print(f"[E2E Smoke Test] Received response: {response}")
    
    if response.get("status") == "ok" and "decision" in response:
        print(f"[E2E Smoke Test] SUCCESS! Laya decided to: {response['decision']}")
    else:
        print("[E2E Smoke Test] FAILED! Invalid response structure.")
        sys.exit(1)
        
    process.kill()

if __name__ == "__main__":
    main()
