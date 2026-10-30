# tests/test_explain.py

import json

from fastapi.testclient import TestClient

from app import app
from explain import build_gemini_payload


client = TestClient(app)


def test_explain_success(monkeypatch):
    def fake_call(payload):
        return json.dumps({
            "summary": "The image includes local metadata and a medium authenticity flag.",
            "risks": ["Location metadata detected", "Device metadata detected"],
            "next_steps": ["Review the metadata", "Remove the flagged details"]
        })

    monkeypatch.setenv("GEMINI_API_KEY", "test-key")
    monkeypatch.setattr("explain._call_gemini", fake_call)

    response = client.post("/explain", json={
        "report": {
            "privacy": [
                {"label": "GPS coordinates", "severity": "High", "value": "40.1234, -73.5678"},
                {"label": "Device make", "severity": "Medium", "value": "Acme Camera"}
            ],
            "authenticity": {"verdict": "Inconclusive", "confidence": 58, "probability": 0.58},
            "regions": {"faces": [], "plates": []},
        }
    })

    assert response.status_code == 200
    payload = response.json()
    assert payload["source"] == "Written by Gemini"
    assert "summary" in payload
    assert len(payload["risks"]) >= 1


def test_explain_malformed_json(monkeypatch):
    monkeypatch.setenv("GEMINI_API_KEY", "test-key")
    monkeypatch.setattr("explain._call_gemini", lambda payload: "not valid json")

    response = client.post("/explain", json={"report": {"privacy": [], "authenticity": {"verdict": "Inconclusive"}}})
    payload = response.json()
    assert response.status_code == 200
    assert "summary" in payload


def test_explain_timeout(monkeypatch):
    monkeypatch.setenv("GEMINI_API_KEY", "test-key")
    monkeypatch.setattr("explain._call_gemini", lambda payload: (_ for _ in ()).throw(TimeoutError("slow")))

    response = client.post("/explain", json={"report": {"privacy": [], "authenticity": {"verdict": "Inconclusive"}}})
    payload = response.json()
    assert response.status_code == 200
    assert payload["source"] == "Built-in explanation"


def test_explain_missing_key():
    response = client.post("/explain", json={"report": {"privacy": [], "authenticity": {"verdict": "Inconclusive"}}})
    payload = response.json()
    assert response.status_code == 200
    assert payload["available"] is False
    assert payload["source"] == "Built-in explanation"


def test_gemini_payload_strips_sensitive_fields():
    payload = {
        "report": {
            "privacy": [
                {"label": "GPS coordinates", "severity": "High", "value": "40.7128, -74.0060"},
                {"label": "Serial number", "severity": "Medium", "value": "ABC123"},
                {"label": "Artist", "severity": "High", "value": "Jane Doe"},
            ],
            "authenticity": {"verdict": "Likely AI-generated", "confidence": 82, "probability": 0.82},
            "image": {"filename": "photo.jpg"},
            "regions": {"faces": [{"x": 10, "y": 20, "w": 30, "h": 40}], "plates": []},
        }
    }
    outbound = json.dumps(build_gemini_payload(payload))
    assert "40.7128" not in outbound
    assert "-74.0060" not in outbound
    assert "ABC123" not in outbound
    assert "Jane Doe" not in outbound
    assert "photo.jpg" not in outbound
    assert "image bytes" not in outbound


if __name__ == "__main__":
    import pytest
    raise SystemExit(pytest.main(["-q", "tests/test_explain.py"]))
