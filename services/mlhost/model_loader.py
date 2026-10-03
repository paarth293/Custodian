"""
model_loader.py — Loads Laya's GGUF model using llama-cpp-python.
Called once at startup. Returns the Llama instance for inference.
"""
import os
from llama_cpp import Llama

_model: Llama | None = None

def load_model(model_name: str = "laya.Q4_K_M.gguf") -> Llama:
    """
    Load the GGUF model from the models/ directory.
    Cached after first load — safe to call multiple times.
    """
    global _model
    if _model is not None:
        return _model

    model_path = os.path.join(
        os.path.dirname(__file__), "..", "..", "models", model_name
    )
    model_path = os.path.abspath(model_path)

    if not os.path.exists(model_path):
        raise FileNotFoundError(
            f"Laya model not found at {model_path}. "
            "Download laya.Q4_K_M.gguf from the Kaggle training output "
            "and place it in the models/ directory."
        )

    print(f"[mlhost] Loading Laya from {model_path} ...")
    _model = Llama(
        model_path=model_path,
        n_ctx=2048,       # Context window
        n_threads=4,      # CPU threads for prefill
        n_gpu_layers=10,  # Offload 10 layers to GPU via Vulkan (GTX 1650)
        verbose=False,
    )
    print("[mlhost] Laya loaded successfully.")
    return _model
