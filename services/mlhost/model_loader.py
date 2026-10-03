import os


def load_model(model_name: str):
    model_path = os.path.join(os.path.dirname(__file__), "..", "..", "models", model_name)
    
    if not os.path.exists(model_path):
        raise FileNotFoundError(f"Model {model_name} not found at {model_path}. You must download it manually.")
        
    print(f"Loading {model_name} from {model_path}...")
    
    # from transformers import AutoModel
    # return AutoModel.from_pretrained(model_path)
    return True
