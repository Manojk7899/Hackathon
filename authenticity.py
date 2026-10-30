import io
from typing import Any, Dict, List

from PIL import Image

try:
    import torch
    from transformers import AutoImageProcessor, AutoModelForImageClassification
except Exception:  # pragma: no cover - dependency may be missing
    torch = None
    AutoImageProcessor = None
    AutoModelForImageClassification = None

MODEL_CANDIDATES = [
    "umm-maybe/AI-image-detector",
    "Organika/sdxl-detector",
]


def _estimate_probability(model_output: Any) -> float | None:
    if model_output is None:
        return None
    logits = getattr(model_output, "logits", None)
    if logits is None:
        return None
    if hasattr(logits, "dim") and logits.dim() > 1:
        probs = torch.softmax(logits, dim=-1)
        return float(probs[0][-1].item()) if probs.shape[0] > 0 else None
    return float(logits[0].item()) if hasattr(logits, "shape") and len(logits.shape) == 1 else None


def analyze_authenticity(file_bytes: bytes, metadata: Dict[str, Any]) -> Dict[str, Any]:
    reasons: List[str] = []
    model_probability = None
    model_status = "model unavailable"
    model_name = ""

    if AutoImageProcessor is not None and AutoModelForImageClassification is not None and torch is not None:
        for candidate in MODEL_CANDIDATES:
            try:
                processor = AutoImageProcessor.from_pretrained(candidate)
                model = AutoModelForImageClassification.from_pretrained(candidate)
                model.to("cpu")
                model.eval()
                image = Image.open(io.BytesIO(file_bytes)).convert("RGB")
                inputs = processor(images=image, return_tensors="pt")
                with torch.no_grad():
                    outputs = model(**inputs)
                    model_probability = _estimate_probability(outputs)
                if model_probability is not None:
                    model_name = candidate
                    model_status = "model loaded"
                    break
            except Exception:
                model_status = "model unavailable"
                model_name = ""
                model_probability = None

    if metadata.get("software"):
        reasons.append(f"Software tag: {metadata['software']}")
    if metadata.get("generator_strings"):
        reasons.append("Known generator or editing strings appear in metadata.")
    if metadata.get("c2pa_marker"):
        reasons.append("C2PA/JUMBF marker detected.")
    if metadata.get("gps"):
        reasons.append("GPS coordinates are embedded in the file.")
    if metadata.get("device"):
        reasons.append("Device information is embedded in EXIF metadata.")
    if metadata.get("thumbnail_present"):
        reasons.append("Embedded thumbnail present.")

    if not any([
        metadata.get("software"),
        metadata.get("generator_strings"),
        metadata.get("gps"),
        metadata.get("device"),
        metadata.get("thumbnail_present"),
        metadata.get("c2pa_marker")
    ]):
        reasons.append("Missing metadata is a weak signal only and does not prove anything by itself.")

    if model_probability is not None:
        if model_probability >= 0.66:
            verdict = "Likely AI-generated"
        elif model_probability >= 0.42:
            verdict = "Inconclusive"
        else:
            verdict = "Unlikely AI-generated"
        confidence = max(15, min(95, int(round(model_probability * 100))))
    else:
        verdict = "Inconclusive"
        confidence = 52 if reasons else 35

    if not reasons:
        reasons.append("No strong metadata or model cues were found in this file.")

    return {
        "verdict": verdict,
        "confidence": confidence,
        "reasons": reasons,
        "model_status": model_status,
        "model_name": model_name,
        "probability": model_probability,
        "note": "Detection is less reliable on heavily compressed images and on generators the model has not seen. Missing metadata is a weak signal only.",
    }


__all__ = ["analyze_authenticity"]
