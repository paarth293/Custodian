"""Offline speech-to-text and text-to-speech. Placeholders."""


def stt(audio_data: bytes) -> str:
    if b"stop" in audio_data:
        return "cubby, stop"
    return ""


def tts(text: str) -> bytes:
    del text
    return b""
