import struct
import sys

from custodian_ipc import read_frame, write_frame


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
            if payload.get("action") == "extract_embeddings":
                resp = write_frame({"status": "ok", "embedding": [0.0] * 1536}, key)
                sys.stdout.buffer.write(resp)
                sys.stdout.buffer.flush()
        except Exception:  # noqa: BLE001, S112
            continue

if __name__ == "__main__":
    process_frames()
