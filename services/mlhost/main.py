import struct
import sys
import traceback

from custodian_ipc import read_frame, write_frame
from laya import decide

def process_frames():
    key = b"super-secret-key"
    
    while True:
        magic = sys.stdin.buffer.read(4)
        if not magic:
            break 
            
        len_bytes = sys.stdin.buffer.read(4)
        if not len_bytes or len(len_bytes) < 4:
            break
            
        length = struct.unpack("<I", len_bytes)[0]
        
        sig = sys.stdin.buffer.read(32)
        payload_bytes = sys.stdin.buffer.read(length)
        
        full_frame = magic + len_bytes + sig + payload_bytes
        
        try:
            payload = read_frame(full_frame, key)
            action = payload.get("action")
            
            if action == "extract_embeddings":
                resp = write_frame({"status": "ok", "embedding": [0.0] * 1536}, key)
                sys.stdout.buffer.write(resp)
                sys.stdout.buffer.flush()
                
            elif action == "decide":
                # Route the decision request to Laya!
                file_data = payload.get("data", {})
                decision = decide(payload.get("d_num", 0), file_data)
                
                resp = write_frame({"status": "ok", "decision": decision}, key)
                sys.stdout.buffer.write(resp)
                sys.stdout.buffer.flush()
                
        except Exception as e:
            # Send back an error frame so Rust doesn't hang
            err_resp = write_frame({"status": "error", "error": str(e)}, key)
            sys.stdout.buffer.write(err_resp)
            sys.stdout.buffer.flush()

if __name__ == "__main__":
    process_frames()
