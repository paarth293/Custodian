import hashlib
import hmac
import json
import struct

MAGIC = b"CUST"
MAX_FRAME_SIZE = 10 * 1024 * 1024

def write_frame(payload: dict, key: bytes) -> bytes:
    json_bytes = json.dumps(payload, separators=(',', ':')).encode('utf-8')
    length = len(json_bytes)
    
    if length > MAX_FRAME_SIZE:
        raise ValueError("Frame too large")
        
    signature = hmac.new(key, json_bytes, hashlib.sha256).digest()
    
    return MAGIC + struct.pack("<I", length) + signature + json_bytes

def read_frame(data: bytes, key: bytes) -> dict:
    if len(data) < 40:
        raise ValueError("Buffer too small")
        
    magic = data[:4]
    if magic != MAGIC:
        raise ValueError("Invalid magic")
        
    length = struct.unpack("<I", data[4:8])[0]
    if length > MAX_FRAME_SIZE:
        raise ValueError("Frame too large")
        
    if len(data) < 40 + length:
        raise ValueError("Incomplete frame")
        
    expected_sig = data[8:40]
    payload_bytes = data[40:40+length]
    
    actual_sig = hmac.new(key, payload_bytes, hashlib.sha256).digest()
    if not hmac.compare_digest(actual_sig, expected_sig):
        raise ValueError("Invalid signature")
        
    return json.loads(payload_bytes.decode('utf-8'))
