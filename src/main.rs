use std::{
    collections::HashMap,
    io::Cursor,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;
use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use dotenvy::dotenv;
use image::{DynamicImage, GenericImage, ImageFormat, Rgba};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    limiter: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl AppState {
    fn allow_request(&self, key: &str) -> bool {
        let mut lock = self.limiter.lock().unwrap();
        let now = Instant::now();
        let entries = lock.entry(key.to_string()).or_insert_with(Vec::new);
        entries.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
        if entries.len() >= 5 {
            return false;
        }
        entries.push(now);
        true
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Finding {
    label: String,
    value: String,
    severity: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Authenticity {
    verdict: String,
    confidence: u32,
    reasons: Vec<String>,
    model_status: String,
    note: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Explanation {
    available: bool,
    source: String,
    summary: String,
    risks: Vec<String>,
    next_steps: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let state = AppState {
        limiter: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/analyze", post(analyze_handler))
        .route("/clean", post(clean_handler))
        .route("/explain", post(explain_handler))
        .fallback_service(ServeDir::new("static"))
        .with_state(state)
        .layer(DefaultBodyLimit::max(15 * 1024 * 1024));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    println!("TruthVision Lens running on http://localhost:8000");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn index() -> Html<String> {
    Html(std::fs::read_to_string("static/index.html").unwrap_or_else(|_| "<h1>TruthVision Lens</h1>".to_string()))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn analyze_handler(mut multipart: Multipart) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut file_name = String::new();
    let mut file_bytes = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            file_name = field.file_name().unwrap_or("upload.jpg").to_string();
            file_bytes = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?.to_vec();
        }
    }

    if file_bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()));
    }

    let lower = file_name.to_ascii_lowercase();
    if !(lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png")) {
        return Err((StatusCode::BAD_REQUEST, "Unsupported image type. Use JPEG or PNG.".to_string()));
    }

    if file_bytes.len() > 15 * 1024 * 1024 {
        return Err((StatusCode::BAD_REQUEST, "File exceeds 15 MB limit.".to_string()));
    }

    let img = image::ImageReader::new(Cursor::new(&file_bytes))
        .with_guessed_format()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Could not decode image: {e}")))?
        .decode()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Unsupported image: {e}")))?;

    let findings = detect_privacy_findings(&file_bytes, &file_name);
    let authenticity = analyze_authenticity(&findings);

    let report = json!({
        "image": {
            "filename": file_name,
            "width": img.width(),
            "height": img.height(),
            "size_bytes": file_bytes.len(),
            "format": if lower.ends_with(".png") { "PNG" } else { "JPEG" }
        },
        "privacy": findings,
        "regions": { "faces": [], "plates": [] },
        "authenticity": authenticity
    });

    Ok((StatusCode::OK, Json(json!({ "report": report }))))
}

async fn clean_handler(mut multipart: Multipart) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut file_name = String::new();
    let mut file_bytes = Vec::new();
    let mut selected_fields: Vec<String> = vec![
        "location".to_string(),
        "device".to_string(),
        "timestamps".to_string(),
        "owner".to_string(),
        "thumbnail".to_string(),
        "everything_else".to_string(),
    ];
    let mut blur_regions = Vec::<HashMap<String, i32>>::new();
    let mut blur_mode = "gaussian".to_string();
    let mut keep_authenticity = false;

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        let value = field.text().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        match name.as_str() {
            "file" => {
                file_name = field.file_name().unwrap_or("upload.jpg").to_string();
                file_bytes = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?.to_vec();
            }
            "selected_fields" => {
                if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
                    selected_fields = parsed;
                }
            }
            "keep_authenticity_fields" => keep_authenticity = value.eq_ignore_ascii_case("true"),
            "blur_regions" => {
                if let Ok(parsed) = serde_json::from_str::<Vec<HashMap<String, i32>>>(&value) {
                    blur_regions = parsed;
                }
            }
            "blur_mode" => blur_mode = value,
            _ => {}
        }
    }

    if file_bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()));
    }

    let img = image::ImageReader::new(Cursor::new(&file_bytes))
        .with_guessed_format()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Could not decode image: {e}")))?
        .decode()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Unsupported image: {e}")))?;

    let cleaned = sanitize_image_bytes(&img, &file_name, &selected_fields, keep_authenticity, &blur_regions, &blur_mode);
    let before = detect_privacy_findings(&file_bytes, &file_name);
    let after = detect_privacy_findings(&cleaned, &file_name);
    let verification = build_verification_rows(&before, &after, &selected_fields, keep_authenticity);

    Ok((
        StatusCode::OK,
        Json(json!({
            "filename": file_name,
            "cleaned_image_base64": STANDARD.encode(cleaned),
            "verification": verification,
            "removed_fields": selected_fields,
            "blur_mode": blur_mode,
            "note": "Re-saving removes any C2PA signature and embedded metadata, and may not erase visible identifiers or sensor fingerprints."
        })),
    ))
}

async fn explain_handler(State(state): State<AppState>, Json(payload): Json<Value>) -> impl IntoResponse {
    let key = "anonymous".to_string();
    if !state.allow_request(&key) {
        return Json(Explanation {
            available: false,
            source: "Built-in explanation".to_string(),
            summary: "Rate limited. Please wait a moment and try again.".to_string(),
            risks: vec!["The request was limited to avoid overuse.".to_string()],
            next_steps: vec!["Wait a moment and try again.".to_string(), "Use the built-in summary if Gemini is unavailable.".to_string()],
        });
    }

    if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
        if !api_key.trim().is_empty() {
            let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash-lite".to_string());
            let client = Client::builder().timeout(Duration::from_secs(15)).build().unwrap();
            let sanitized = build_gemini_payload(&payload);
            let req = client
                .post(format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"))
                .json(&json!({
                    "contents": [{"parts": [{"text": serde_json::to_string(&sanitized).unwrap_or_default()}]}],
                    "generationConfig": { "temperature": 0.2, "responseMimeType": "application/json" }
                }))
                .send();

            match req.await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(text) = resp.text().await {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                            let summary = parsed
                                .get("candidates")
                                .and_then(|c| c.as_array())
                                .and_then(|a| a.first())
                                .and_then(|c| c.get("content"))
                                .and_then(|c| c.get("parts"))
                                .and_then(|p| p.as_array())
                                .and_then(|p| p.first())
                                .and_then(|p| p.get("text"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("The file contains a mix of privacy and authenticity cues.");

                            return Json(Explanation {
                                available: true,
                                source: "Written by Gemini".to_string(),
                                summary: summary.to_string(),
                                risks: vec!["The verdict is based on metadata and heuristic cues.".to_string()],
                                next_steps: vec![
                                    "Review the flagged metadata before sharing the file.".to_string(),
                                    "Remove the selected metadata groups if they are not needed.".to_string(),
                                    "Keep the cleaned file local and check the authenticity verdict again.".to_string(),
                                ],
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Json(build_builtin_explanation(&payload))
}

fn build_gemini_payload(payload: &Value) -> Value {
    let report = payload.get("report").unwrap_or(payload);
    let privacy = report.get("privacy").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let authenticity = report.get("authenticity").cloned().unwrap_or(Value::Null);

    let risks = privacy
        .iter()
        .filter_map(|item| item.get("label").and_then(|v| v.as_str()))
        .filter(|label| {
            let lower = label.to_ascii_lowercase();
            lower.contains("gps")
                || lower.contains("serial")
                || lower.contains("artist")
                || lower.contains("author")
                || lower.contains("device")
                || lower.contains("timestamp")
                || lower.contains("thumbnail")
                || lower.contains("owner")
                || lower.contains("generator")
                || lower.contains("png")
        })
        .map(|label| label.to_string())
        .take(4)
        .collect::<Vec<_>>();

    let verdict = authenticity.get("verdict").and_then(|v| v.as_str()).unwrap_or("Inconclusive");
    let confidence = authenticity.get("confidence").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let bucket = if confidence >= 70 { "high" } else if confidence >= 40 { "medium" } else { "low" };

    json!({
        "summary": format!("The authenticity likelihood is {verdict}. The model probability is {bucket} confidence, and detection is less reliable on compressed images and unseen generators. Missing metadata is a weak signal only."),
        "risks": if risks.is_empty() { vec!["No clear privacy signals were found."] } else { risks },
        "next_steps": [
            "Review the flagged metadata before sharing the image.",
            "Remove the selected metadata groups if they are not needed.",
            "Re-check the authenticity verdict after cleaning."
        ]
    })
}

fn build_builtin_explanation(payload: &Value) -> Explanation {
    let report = payload.get("report").unwrap_or(payload);
    let privacy = report.get("privacy").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let authenticity = report.get("authenticity").cloned().unwrap_or(Value::Null);
    let verdict = authenticity.get("verdict").and_then(|v| v.as_str()).unwrap_or("Inconclusive");

    let risks = privacy
        .iter()
        .filter_map(|item| item.get("label").and_then(|v| v.as_str()))
        .take(4)
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    Explanation {
        available: false,
        source: "Built-in explanation".to_string(),
        summary: format!("The current authenticity likelihood is {verdict}. Detection is heuristic and less reliable on heavily compressed images and unseen generators. Missing metadata is a weak signal only, and the report focuses on metadata that is present."),
        risks: if risks.is_empty() { vec!["No clear privacy signals were found.".to_string()] } else { risks },
        next_steps: vec![
            "Review the flagged metadata before sharing the file.".to_string(),
            "Remove the selected metadata groups if they are not needed.".to_string(),
            "Keep the cleaned file local and check the authenticity verdict again.".to_string(),
        ],
    }
}

fn detect_privacy_findings(file_bytes: &[u8], file_name: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let lower_name = file_name.to_ascii_lowercase();
    let bytes = String::from_utf8_lossy(file_bytes).to_ascii_lowercase();

    if bytes.contains("gps") || bytes.contains("latitude") || bytes.contains("longitude") {
        findings.push(Finding {
            label: "GPS coordinates".to_string(),
            value: "Location embedded in file".to_string(),
            severity: "High".to_string(),
        });
    }

    if bytes.contains("serial") || bytes.contains("device serial") {
        findings.push(Finding {
            label: "Serial number".to_string(),
            value: "Device serial present".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if bytes.contains("artist") || bytes.contains("author") || bytes.contains("copyright") {
        findings.push(Finding {
            label: "Owner or artist metadata".to_string(),
            value: "Personal attribution present".to_string(),
            severity: "High".to_string(),
        });
    }

    if bytes.contains("model") || bytes.contains("make") || bytes.contains("camera") {
        findings.push(Finding {
            label: "Device make and model".to_string(),
            value: "Camera metadata present".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if bytes.contains("202") || bytes.contains("datetime") || bytes.contains("date") {
        findings.push(Finding {
            label: "Timestamps".to_string(),
            value: "Embedded EXIF time present".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if lower_name.ends_with(".png") && (bytes.contains("software") || bytes.contains("comment") || bytes.contains("creation time")) {
        findings.push(Finding {
            label: "PNG text metadata".to_string(),
            value: "Possible text chunks detected".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if bytes.contains("adobe") || bytes.contains("photoshop") || bytes.contains("lightroom") || bytes.contains("midjourney") || bytes.contains("stable diffusion") || bytes.contains("dall-e") {
        findings.push(Finding {
            label: "Generator or editing metadata".to_string(),
            value: "Known editing strings detected".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if bytes.contains("c2pa") || bytes.contains("jumbf") {
        findings.push(Finding {
            label: "C2PA marker".to_string(),
            value: "Marker detected".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if findings.is_empty() {
        findings.push(Finding {
            label: "No clear metadata leaks".to_string(),
            value: "No obvious private metadata found".to_string(),
            severity: "Low".to_string(),
        });
    }

    let mut seen = HashMap::new();
    let mut deduped = Vec::new();
    for item in findings {
        if seen.insert(item.label.clone(), true).is_none() {
            deduped.push(item);
        }
    }
    deduped
}

fn analyze_authenticity(findings: &[Finding]) -> Authenticity {
    let mut score = 0.0f32;
    for finding in findings {
        let label = finding.label.to_ascii_lowercase();
        if label.contains("gps") { score += 0.22; }
        if label.contains("serial") || label.contains("device") { score += 0.14; }
        if label.contains("owner") || label.contains("artist") { score += 0.18; }
        if label.contains("timestamp") { score += 0.08; }
        if label.contains("png") || label.contains("generator") || label.contains("c2pa") { score += 0.1; }
    }

    let probability = (score / 1.5).clamp(0.06, 0.94);
    let verdict = if probability >= 0.66 {
        "Likely AI-generated".to_string()
    } else if probability >= 0.42 {
        "Inconclusive".to_string()
    } else {
        "Unlikely AI-generated".to_string()
    };

    Authenticity {
        verdict: verdict.clone(),
        confidence: (probability * 100.0).round() as u32,
        reasons: vec![
            "Metadata and heuristic signals were considered.".to_string(),
            "Missing metadata is a weak signal only.".to_string(),
            "Detection is less reliable on heavily compressed images and unseen generators.".to_string(),
            "Model access is unavailable in this local prototype.".to_string(),
        ],
        model_status: "model unavailable".to_string(),
        note: format!("This local verdict is heuristic only. Current result: {verdict}."),
    }
}

fn sanitize_image_bytes(
    img: &DynamicImage,
    _file_name: &str,
    selected_fields: &[String],
    _keep_authenticity: bool,
    blur_regions: &[HashMap<String, i32>],
    blur_mode: &str,
) -> Vec<u8> {
    let mut rgba = img.to_rgba8();

    for region in blur_regions {
        if let (Some(x), Some(y), Some(w), Some(h)) = (region.get("x"), region.get("y"), region.get("w"), region.get("h")) {
            for yy in *y..(*y + *h).min(rgba.height() as i32) {
                for xx in *x..(*x + *w).min(rgba.width() as i32) {
                    if yy >= 0 && xx >= 0 {
                        let px = rgba.get_pixel(xx as u32, yy as u32);
                        let mut rgb = px.0;
                        if blur_mode == "pixelate" {
                            let avg = ((rgb[0] as u16 + rgb[1] as u16 + rgb[2] as u16) / 3) as u8;
                            rgb[0] = avg;
                            rgb[1] = avg;
                            rgb[2] = avg;
                        } else {
                            rgb[0] = rgb[0].saturating_sub(32);
                            rgb[1] = rgb[1].saturating_sub(32);
                            rgb[2] = rgb[2].saturating_sub(32);
                        }
                        rgba.put_pixel(xx as u32, yy as u32, Rgba([rgb[0], rgb[1], rgb[2], 255]));
                    }
                }
            }
        }
    }

    let mut cursor = Cursor::new(Vec::new());
    let format = if selected_fields.iter().any(|v| v == "location" || v == "device" || v == "timestamps" || v == "owner") {
        ImageFormat::Jpeg
    } else {
        ImageFormat::Png
    };

    rgba.write_to(&mut cursor, format).unwrap();
    cursor.into_inner()
}

fn build_verification_rows(before: &[Finding], after: &[Finding], selected_fields: &[String], _keep_authenticity: bool) -> Vec<Value> {
    let mut rows = Vec::new();
    let field_map = [
        ("location", "GPS coordinates"),
        ("device", "Device details"),
        ("timestamps", "Timestamps"),
        ("owner", "Owner fields"),
        ("thumbnail", "Embedded thumbnail"),
        ("everything_else", "Metadata traces"),
    ];

    for (key, label) in field_map {
        let selected = selected_fields.iter().any(|v| v == key);
        let before_present = before.iter().any(|f| {
            let lower = f.label.to_ascii_lowercase();
            lower.contains(key) || (key == "location" && lower.contains("gps"))
        });
        let after_present = after.iter().any(|f| {
            let lower = f.label.to_ascii_lowercase();
            lower.contains(key) || (key == "location" && lower.contains("gps"))
        });

        let status = if selected && before_present && !after_present {
            "Removed"
        } else {
            "Kept"
        };

        rows.push(json!({
            "field": label,
            "before": if before_present { "Present" } else { "Not present" },
            "after": if after_present { "Kept" } else { "Removed" },
            "status": status
        }));
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_payload_strips_sensitive_fields() {
        let payload = json!({
            "report": {
                "privacy": [
                    {"label": "GPS coordinates", "severity": "High", "value": "40.7128, -74.0060"},
                    {"label": "Serial number", "severity": "Medium", "value": "ABC123"},
                    {"label": "Artist", "severity": "High", "value": "Jane Doe"}
                ],
                "authenticity": {"verdict": "Likely AI-generated", "confidence": 82}
            }
        });

        let outbound = serde_json::to_string(&build_gemini_payload(&payload)).unwrap();
        assert!(!outbound.contains("40.7128"));
        assert!(!outbound.contains("-74.0060"));
        assert!(!outbound.contains("ABC123"));
        assert!(!outbound.contains("Jane Doe"));
        assert!(!outbound.contains("photo.jpg"));
        assert!(!outbound.contains("image bytes"));
    }
}
