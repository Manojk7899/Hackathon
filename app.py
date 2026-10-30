import json
import os
import time
from collections import defaultdict, deque
from typing import Any, Dict, List

from dotenv import load_dotenv

load_dotenv()

DEFAULT_MODEL = os.getenv("GEMINI_MODEL", "gemini-2.5-flash")
GEMINI_TIMEOUT_SECONDS = 15
RATE_LIMIT_CALLS = 3
RATE_LIMIT_WINDOW_SECONDS = 60

try:
    from google import genai
except Exception:  # pragma: no cover
    genai = None

REQUEST_TIMESTAMPS: Dict[str, deque] = defaultdict(deque)


def _get_client_ip(client_ip: str | None) -> str:
    return client_ip or "anonymous"


def _rate_limit_allowed(client_ip: str | None) -> bool:
    client_id = _get_client_ip(client_ip)
    now = time.monotonic()
    bucket = REQUEST_TIMESTAMPS[client_id]
    while bucket and now - bucket[0] > RATE_LIMIT_WINDOW_SECONDS:
        bucket.popleft()
    if len(bucket) >= RATE_LIMIT_CALLS:
        return False
    bucket.append(now)
    return True


def summarize_findings(payload: Dict[str, Any]) -> Dict[str, Any]:
    report = payload.get("report", payload)
    privacy = report.get("privacy", report.get("findings", []))
    authenticity = report.get("authenticity", {})
    regions = report.get("regions", {})
    face_count = len(regions.get("faces", []))
    plate_count = len(regions.get("plates", []))

    findings: List[str] = []
    if isinstance(privacy, list):
        for item in privacy:
            if not isinstance(item, dict):
                continue
            label = str(item.get("label", "")).strip()
            severity = str(item.get("severity", "")).strip()
            if label and severity:
                findings.append(f"{label}: {severity.lower()} severity")

    model_bucket = "unknown"
    if isinstance(authenticity, dict):
        if authenticity.get("model_status") == "model unavailable":
            model_bucket = "unknown"
        else:
            probability = authenticity.get("probability")
            if probability is not None:
                if probability >= 0.66:
                    model_bucket = "high"
                elif probability >= 0.42:
                    model_bucket = "medium"
                else:
                    model_bucket = "low"

    summary_bits = []
    if findings:
        summary_bits.append("The file contains privacy signals including metadata and location-related details")
    if face_count:
        summary_bits.append(f"{face_count} face region(s) were detected")
    if plate_count:
        summary_bits.append(f"{plate_count} plate region(s) were detected")
    if not summary_bits:
        summary_bits.append("The file appears to have minimal embedded metadata")

    summary = " ".join(summary_bits) + ". " + (
        f"Authenticity is {authenticity.get('verdict', 'Inconclusive').lower()} with {authenticity.get('confidence', 0)}% confidence and a {model_bucket} model signal."
    )

    risks = []
    if findings:
        risks.extend(findings[:3])
    if authenticity.get("verdict"):
        risks.append(f"Authenticity verdict: {authenticity.get('verdict')}")
    if not risks:
        risks.append("No strong privacy or authenticity signals were found.")

    next_steps = [
        "Review the selected metadata groups before saving a cleaned copy.",
        "Remove the flagged location and owner details if they are not needed.",
        "Keep the result file local and avoid sharing it until the privacy risks are checked."
    ]

    return {
        "summary": summary,
        "risks": risks[:4],
        "next_steps": next_steps[:3],
    }


def build_builtin_explanation(payload: Dict[str, Any]) -> Dict[str, Any]:
    sanitized = summarize_findings(payload)
    return {
        "available": False,
        "source": "Built-in explanation",
        "summary": sanitized["summary"],
        "risks": sanitized["risks"],
        "next_steps": sanitized["next_steps"],
    }


def _safe_json_dumps(data: Dict[str, Any]) -> str:
    return json.dumps(data, ensure_ascii=True, separators=(",", ":"))


def build_gemini_payload(payload: Dict[str, Any]) -> Dict[str, Any]:
    sanitized = summarize_findings(payload)
    return {
        "summary": sanitized["summary"],
        "risks": sanitized["risks"],
        "next_steps": sanitized["next_steps"],
    }


def validate_explanation_response(data: Any) -> Dict[str, Any]:
    if not isinstance(data, dict):
        return {"summary": str(data), "risks": [], "next_steps": []}
    summary = str(data.get("summary", "")).strip()
    risks = data.get("risks", [])
    next_steps = data.get("next_steps", [])
    if not isinstance(risks, list):
        risks = []
    if not isinstance(next_steps, list):
        next_steps = []
    if not summary:
        summary = "The system could not generate a structured explanation for this report."
    return {
        "summary": summary,
        "risks": [str(item) for item in risks[:4]],
        "next_steps": [str(item) for item in next_steps[:3]],
    }


def _call_gemini(payload: Dict[str, Any]) -> str:
    api_key = os.getenv("GEMINI_API_KEY", "").strip()
    if not api_key:
        raise RuntimeError("Missing GEMINI_API_KEY")
    if genai is None:
        raise RuntimeError("google-genai is not installed")

    client = genai.Client(api_key=api_key)
    system_instruction = (
        "Explain for a non-technical reader. Stay calibrated. Never say an image is definitely real or fake. "
        "Describe verdicts as likelihoods. Mention that missing metadata is a weak signal and that detection is less reliable on compressed images and unseen generators when relevant. "
        "Invent nothing beyond the provided findings. No markdown headers, no emoji, no exclamation marks. About 180 words maximum."
    )
    request = _safe_json_dumps(payload)
    response = client.models.generate_content(
        model=os.getenv("GEMINI_MODEL", DEFAULT_MODEL),
        contents=request,
        config={"system_instruction": system_instruction, "temperature": 0.2},
    )
    if hasattr(response, "text"):
        return str(response.text)
    if hasattr(response, "candidates"):
        for candidate in response.candidates:
            if hasattr(candidate, "content"):
                return str(candidate.content)
    return str(response)


def explain_findings(payload: Dict[str, Any], client_ip: str | None = None) -> Dict[str, Any]:
    if not isinstance(payload, dict):
        return build_builtin_explanation({})

    sanitized = build_gemini_payload(payload)
    api_key = os.getenv("GEMINI_API_KEY", "").strip()
    if not api_key:
        result = build_builtin_explanation(payload)
        result["available"] = False
        return result

    if not _rate_limit_allowed(client_ip):
        result = build_builtin_explanation(payload)
        result["available"] = False
        result["summary"] = "The explanation service is temporarily rate-limited. The built-in explanation is shown instead."
        return result

    try:
        raw = _call_gemini(sanitized)
    except TimeoutError:
        result = build_builtin_explanation(payload)
        result["available"] = False
        result["summary"] = "The explanation request timed out. The built-in explanation is shown instead."
        return result
    except Exception:
        result = build_builtin_explanation(payload)
        result["available"] = False
        result["summary"] = "The explanation service is unavailable. The built-in explanation is shown instead."
        return result

    try:
        parsed = json.loads(raw)
    except (TypeError, ValueError):
        mapped = validate_explanation_response({"summary": raw, "risks": [], "next_steps": []})
        return {
            "available": True,
            "source": "Written by Gemini",
            **mapped,
        }

    validated = validate_explanation_response(parsed)
    return {
        "available": True,
        "source": "Written by Gemini",
        **validated,
    }


__all__ = [
    "build_builtin_explanation",
    "build_gemini_payload",
    "explain_findings",
    "validate_explanation_response",
]
