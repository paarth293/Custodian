def stt(audio_data: bytes) -> str:
    if b"stop" in audio_data:
        return "cubby, stop"
    return "mock transcript"

def tts(text: str) -> bytes:
    return b"mock audio bytes"
