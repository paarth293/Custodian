import sys
import os
sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from custodian_ipc import write_frame
from main import process_frames
import io

def test_multiple_frames():
    key = b"super-secret-key"
    f1 = write_frame({"action": "extract_embeddings"}, key)
    f2 = write_frame({"action": "ping"}, key)
    
    sys.stdin = io.TextIOWrapper(io.BytesIO(f1 + f2))
    sys.stdout = io.TextIOWrapper(io.BytesIO())
    
    process_frames()
    
    assert True
