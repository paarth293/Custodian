"""Loads Laya's GGUF model with llama-cpp-python (optional dependency)."""

from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Any

DEFAULT_MODEL = "laya.Q4_K_M.gguf"
_REPO_ROOT = Path(__file__).resolve().parents[3]

_model: Any = None


def models_dir() -> Path:
    """Directory holding model files; override with ``CUSTODIAN_MODELS_DIR``."""
    return Path(os.environ.get("CUSTODIAN_MODELS_DIR", _REPO_ROOT / "models"))


def load_model(model_name: str = DEFAULT_MODEL) -> Any:
    """Loads the model once and caches it for subsequent calls."""
    global _model
    if _model is not None:
        return _model

    model_path = models_dir() / model_name
    if not model_path.exists():
        raise FileNotFoundError(
            f"Laya model not found at {model_path}. "
            "Place the fine-tuned GGUF file in the models/ directory."
        )

    try:
        from llama_cpp import Llama
    except ImportError as exc:
        raise RuntimeError("llama-cpp-python is not installed; see requirements-model.txt") from exc

    print(f"[mlhost] Loading Laya from {model_path} ...", file=sys.stderr)
    _model = Llama(
        model_path=str(model_path),
        n_ctx=2048,
        n_threads=4,
        n_gpu_layers=10,
        verbose=False,
    )
    return _model
