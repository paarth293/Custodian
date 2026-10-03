"""
laya.py — The core decision engine using the fine-tuned GGUF model.
"""
import json
import traceback
from model_loader import load_model

def decide(d_num: int, data: dict) -> str:
    """
    Takes file metadata/scan results and asks Laya for a decision.
    Returns: "approve", "delete", "quarantine", or "abstained"
    """
    try:
        model = load_model()
        
        # Build the exact prompt format we fine-tuned her on
        filename = data.get("filename", "unknown")
        av_hits = data.get("av_hits", 0)
        signed = data.get("signed", False)
        
        user_prompt = f"Analyze file for security decision:\nFilename: {filename}\nAV Hits: {av_hits}\nSigned: {signed}"
        
        messages = [
            {"role": "user", "content": user_prompt}
        ]
        
        # Ask Laya
        response = model.create_chat_completion(
            messages=messages,
            max_tokens=100,
            temperature=0.1,  # Keep it deterministic
            response_format={"type": "json_object"}, # Force strict JSON
        )
        
        # Parse the JSON response
        content = response["choices"][0]["message"]["content"]
        result = json.loads(content)
        
        action = result.get("action")
        if action in ["approve", "delete", "quarantine"]:
            return action
        else:
            print(f"[mlhost] Laya returned unknown action: {action}")
            return "abstained"
            
    except Exception as e:
        print(f"[mlhost] Laya decision failed: {e}")
        traceback.print_exc()
        return "abstained"
