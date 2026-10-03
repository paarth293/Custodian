def stt(audio_data):
    if b"stop" in audio_data:
        return "cubby, stop"
    return "mock transcript"

def tts(text):
    return b"mock audio bytes"
