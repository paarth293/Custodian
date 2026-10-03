def tokenize_text(text: str) -> list[int]:
    print(f"Tokenizing: {text}")
    tokens = [ord(c) for c in text]
    print(f"Tokens: {tokens}")
    return tokens

if __name__ == "__main__":
    tokenize_text("Hello Custodian")
