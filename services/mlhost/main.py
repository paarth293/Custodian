import struct
import sys

from custodian_ipc import MAX_FRAME_SIZE, read_frame, write_frame


def process_frames() -> None:
    key = b"super-secret-key"

    while True:
        magic = sys.stdin.buffer.read(4)
        if not magic:
            break

        len_bytes = sys.stdin.buffer.read(4)
        if not len_bytes or len(len_bytes) < 4:
            break

        length = struct.unpack("<I", len_bytes)[0]
        if length > MAX_FRAME_SIZE:
            # Refuse to buffer an oversized payload; the stream cannot be resynced.
            print("E_FRAME: frame too large", file=sys.stderr)
            break

        sig = sys.stdin.buffer.read(32)
        payload_bytes = sys.stdin.buffer.read(length)

        full_frame = magic + len_bytes + sig + payload_bytes

        try:
            payload = read_frame(full_frame, key)
            if payload.get("action") == "extract_embeddings":
                resp = write_frame({"status": "ok", "embedding": [0.0] * 1536}, key)
                sys.stdout.buffer.write(resp)
                sys.stdout.buffer.flush()
        except ValueError as e:
            print(f"E_FRAME: {e}", file=sys.stderr)


if __name__ == "__main__":
    process_frames()
