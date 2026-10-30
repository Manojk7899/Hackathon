use std::{
    collections::HashMap,
    io::Cursor,
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
use image::{DynamicImage, GenericImage, ImageOutputFormat, Rgba, RgbaImage};
use kamadak_exif as exif;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    rate_limiter: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl AppState {
    fn allow_request(&self, key: &str) -> bool {
        let mut lock = self.rate_limiter.lock().unwrap();
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ReportEnvelope {
    report: Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let state = AppState {
        rate_limiter: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/analyze", post(analyze_handler))
        .route("/clean", post(clean_handler))
        .route("/explain", post(explain_handler))
        .fallback_service(ServeDir::new("static").fallback(index))
        .with_state(state)
        .layer(DefaultBodyLimit::max(15 * 1024 * 1024));

    println!("TruthVision Lens running on http://localhost:8000");
    axum::serve(
        tokio::net::TcpListener::bind("0.0.0.0:8000").await?,
        app,
    )
    .await?;
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
    let authenticity = analyze_authenticity(&file_bytes, &findings);

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
    let mut selected_fields = vec!["location", "device", "timestamps", "owner", "thumbnail", "everything_else"];
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
    let ok = state.allow_request(&key);
    if !ok {
        return Json(Explanation {
            available: false,
            source: "Built-in explanation".to_string(),
            summary: "The explanation feature is temporarily rate-limited. Please wait a moment and try again.".to_string(),
            risks: vec!["The request was limited to avoid overuse.".to_string()],
            next_steps: vec!["Wait a moment and try again.".to_string(), "Use the built-in summary if Gemini is unavailable.".to_string()],
        });
    }

    let sanitized = build_gemini_payload(&payload);
    let api_key = std::env::var("GEMINI_API_KEY").ok().filter(|v| !v.trim().is_empty());

    if let Some(api_key) = api_key {
        let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash-lite".to_string());

        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().unwrap();
        let request_body = json!({
            "system_instruction": {
                "parts": [{
                    "text": "Explain for a non-technical reader, stay calibrated, never say an image is definitely real or fake, describe verdicts as likelihoods, mention that missing metadata is a weak signal and detection is less reliable on compressed images and unseen generators when relevant, invent nothing beyond the provided findings, no markdown headers, no emoji, no exclamation marks, about 180 words maximum."
                }]
            },
            "contents": [{
                "parts": [{
                    "text": serde_json::to_string(&sanitized).unwrap_or_default()
                }]
            }],
            "generationConfig": {
                "temperature": 0.2,
                "responseMimeType": "application/json"
            }
        });

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}");
        match client.post(url).json(&request_body).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                            let summary = parsed.get("candidates")
                                .and_then(|c| c.as_array())
                                .and_then(|a| a.first())
                                .and_then(|c| c.get("content"))
                                .and_then(|c| c.get("parts"))
                                .and_then(|p| p.as_array())
                                .and_then(|p| p.first())
                                .and_then(|p| p.get("text"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("The image contains a limited set of privacy and authenticity cues.");

                            let result = parse_explanation_text(summary);
                            return Json(result);
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }

    Json(build_builtin_explanation(&payload))
}

fn build_gemini_payload(payload: &Value) -> Value {
    let report = payload.get("report").unwrap_or(payload);
    let privacy = report
        .get("privacy")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let authenticity = report.get("authenticity").cloned().unwrap_or(Value::Null);

    let mut risks = Vec::new();
    let mut risks_text = Vec::new();
    for item in privacy {
        if let Some(label) = item.get("label").and_then(|v| v.as_str()) {
            let lower = label.to_ascii_lowercase();
            let is_sensitive = lower.contains("gps")
                || lower.contains("serial")
                || lower.contains("artist")
                || lower.contains("author")
                || lower.contains("copyright")
                || lower.contains("device")
                || lower.contains("timestamp")
                || lower.contains("thumbnail")
                || lower.contains("owner")
                || lower.contains("software")
                || lower.contains("generator")
                || lower.contains("png text");
            if is_sensitive {
                let severity = item.get("severity").and_then(|v| v.as_str()).unwrap_or("Medium");
                let display = format!("{label} ({severity})");
                risks.push(display.clone());
                risks_text.push(display);
            }
        }
    }

    if risks_text.is_empty() {
        risks_text.push("No clear privacy signals were found.".to_string());
    }

    let verdict = authenticity.get("verdict").and_then(|v| v.as_str()).unwrap_or("Inconclusive");
    let confidence = authenticity.get("confidence").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let bucket = if confidence >= 70 { "high" } else if confidence >= 40 { "medium" } else { "low" };

    json!({
        "summary": format!("The overall authenticity likelihood is {verdict}. The model probability is {bucket} confidence, and detection is less reliable on heavily compressed images and unseen generators. Missing metadata is a weak signal only."),
        "risks": risks_text,
        "next_steps": ["Review the flagged metadata before sharing the image.", "Remove the selected metadata groups if they are not needed.", "Re-check the authenticity verdict after cleaning."]
    })
}

fn parse_explanation_text(raw: &str) -> Explanation {
    let text = raw.trim();
    let mut summary = text.to_string();
    if summary.is_empty() {
        summary = "The file appears to have a mix of metadata and authenticity signals, and the verdict remains a likelihood estimate rather than a definitive fact.".to_string();
    }

    let risks = vec![
        "The verdict is based on metadata and heuristic cues, not a forensic guarantee.".to_string(),
        "Missing metadata is a weak signal and should not be treated as proof.".to_string(),
    ];

    Explanation {
        available: true,
        source: "Written by Gemini".to_string(),
        summary,
        risks,
        next_steps: vec![
            "Review the flagged metadata before sharing the file.".to_string(),
            "Remove the private location and owner details if they are not needed.".to_string(),
            "Keep the cleaned file local and use the authenticity verdict as guidance only.".to_string(),
        ],
    }
}

fn build_builtin_explanation(payload: &Value) -> Explanation {
    let report = payload.get("report").unwrap_or(payload);
    let privacy = report.get("privacy").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let authenticity = report.get("authenticity").cloned().unwrap_or(Value::Null);
    let verdict = authenticity.get("verdict").and_then(|v| v.as_str()).unwrap_or("Inconclusive");
    let summary = format!("The current authenticity likelihood is {verdict}. Detection is heuristic and less reliable on heavily compressed images and unseen generators. Missing metadata is a weak signal only, and the report focuses on the metadata that is present.");

    let risks = privacy
        .iter()
        .filter_map(|item| item.get("label").and_then(|v| v.as_str()).map(str::to_string))
        .take(4)
        .collect::<Vec<_>>();

    let explanation = Explanation {
        available: false,
        source: "Built-in explanation".to_string(),
        summary,
        risks: if risks.is_empty() { vec!["No strong privacy signals were found.".to_string()] } else { risks },
        next_steps: vec![
            "Review the flagged metadata before sharing the file.".to_string(),
            "Remove the selected metadata groups if they are not needed.".to_string(),
            "Keep the cleaned file local and check the authenticity verdict again.".to_string(),
        ],
    };
    explanation
}

fn detect_privacy_findings(file_bytes: &[u8], file_name: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let text = String::from_utf8_lossy(file_bytes).to_ascii_lowercase();
    let lower_name = file_name.to_ascii_lowercase();

    if let Ok(exif_reader) = exif::Reader::new().read_from_container(&mut Cursor::new(file_bytes)) {
        for field in exif_reader.fields() {
            let tag = field.tag;
            if tag == 1 || tag == 2 || tag == 3 || tag == 4 {
                findings.push(Finding {
                    label: "GPS coordinates".to_string(),
                    value: "Location embedded in file".to_string(),
                    severity: "High".to_string(),
                });
                break;
            }
            if tag == 271 || tag == 272 || tag == 0x000A || tag == 0x000B {
                findings.push(Finding {
                    label: "Device make and model".to_string(),
                    value: "Camera metadata present".to_string(),
                    severity: "Medium".to_string(),
                });
            }
            if tag == 306 || tag == 36867 || tag == 36868 {
                findings.push(Finding {
                    label: "Timestamps".to_string(),
                    value: "Embedded EXIF time present".to_string(),
                    severity: "Medium".to_string(),
                });
            }
            if tag == 315 || tag == 33432 || tag == 0x0009 {
                findings.push(Finding {
                    label: "Owner or artist metadata".to_string(),
                    value: "Personal attribution present".to_string(),
                    severity: "High".to_string(),
                });
            }
        }
    }

    if text.contains("gps") || text.contains("latitude") || text.contains("longitude") {
        if !findings.iter().any(|f| f.label == "GPS coordinates") {
            findings.push(Finding {
                label: "GPS coordinates".to_string(),
                value: "Location embedded in file".to_string(),
                severity: "High".to_string(),
            });
        }
    }

    if text.contains("serial") || text.contains("device serial") {
        findings.push(Finding {
            label: "Serial number".to_string(),
            value: "Device serial present".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if lower_name.ends_with(".png") && (text.contains("software") || text.contains("artist") || text.contains("comment") || text.contains("creation time")) {
        findings.push(Finding {
            label: "PNG text metadata".to_string(),
            value: "Possible text chunks detected".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if lower_name.ends_with(".jpg") || lower_name.ends_with(".jpeg") {
        if !findings.iter().any(|f| f.label == "Timestamps") {
            findings.push(Finding {
                label: "Timestamps".to_string(),
                value: "Embedded EXIF time present".to_string(),
                severity: "Medium".to_string(),
            });
        }
    }

    if text.contains("adobe") || text.contains("photoshop") || text.contains("lightroom") || text.contains("midjourney") || text.contains("stable diffusion") || text.contains("dall-e") || text.contains("firefly") {
        findings.push(Finding {
            label: "Generator or editing metadata".to_string(),
            value: "Known creator or editing strings detected".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if text.contains("c2pa") || text.contains("jumbf") {
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

    let mut dedup = Vec::new();
    let mut seen = HashMap::new();
    for item in findings {
        if seen.insert(item.label.clone(), true).is_none() {
            dedup.push(item);
        }
    }
    dedup
}

fn analyze_authenticity(file_bytes: &[u8], findings: &[Finding]) -> Authenticity {
    let mut score = 0.0f32;
    for finding in findings {
        let label = finding.label.to_ascii_lowercase();
        if label.contains("gps") {
            score += 0.22;
        }
        if label.contains("serial") || label.contains("device") {
            score += 0.14;
        }
        if label.contains("owner") || label.contains("artist") {
            score += 0.18;
        }
        if label.contains("timestamp") {
            score += 0.08;
        }
        if label.contains("png text") || label.contains("generator") || label.contains("c2pa") {
            score += 0.1;
        }
    }

    let probability = (score / 1.5).clamp(0.06, 0.94);
    let verdict = if probability >= 0.66 {
        "Likely AI-generated".to_string()
    } else if probability >= 0.42 {
        "Inconclusive".to_string()
    } else {
        "Unlikely AI-generated".to_string()
    };

    let confidence = (probability * 100.0).round() as u32;
    Authenticity {
        verdict: verdict.clone(),
        confidence,
        reasons: vec![
            "Metadata and heuristic signals were considered.".to_string(),
            "Missing metadata is a weak signal only.".to_string(),
            "Detection is less reliable on heavily compressed images and unseen generators.".to_string(),
            "No model was loaded in this local prototype.".to_string(),
        ],
        model_status: "model unavailable".to_string(),
        note: format!("This app uses a heuristic verdict. The current result is {verdict}, not a forensic claim."),
    }
}

fn sanitize_image_bytes(
    img: &DynamicImage,
    file_name: &str,
    selected_fields: &[String],
    keep_authenticity: bool,
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
                            let average = ((rgb[0] as u16 + rgb[1] as u16 + rgb[2] as u16) / 3) as u8;
                            rgb[0] = average;
                            rgb[1] = average;
                            rgb[2] = average;
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
    let format = if file_name.to_ascii_lowercase().ends_with(".png") {
        ImageOutputFormat::Png
    } else {
        ImageOutputFormat::Jpeg(92)
    };
    rgba.write_to(&mut cursor, format).unwrap();
    let mut output = cursor.into_inner();

    if !keep_authenticity {
        output = cursor.into_inner();
    }

    output
}

fn build_verification_rows(before: &[Finding], after: &[Finding], selected_fields: &[String], keep_authenticity: bool) -> Vec<Value> {
    let mut rows = Vec::new();
    let field_map = [
        ("location", "GPS coordinates"),
        ("device", "Device details"),
        ("timestamps", "Timestamps"),
        ("owner", "Owner fields"),
        ("thumbnail", "Embedded thumbnail"),
        ("everything_else", "Metadata traces"),
        ("authenticity", "Authenticity metadata"),
    ];

    for (key, label) in field_map {
        let selected = selected_fields.iter().any(|v| v == key);
        let before_present = before.iter().any(|f| f.label.to_ascii_lowercase().contains(key) || f.label.to_ascii_lowercase().contains("gps") && key == "location");
        let after_present = after.iter().any(|f| f.label.to_ascii_lowercase().contains(key) || f.label.to_ascii_lowercase().contains("gps") && key == "location");

        let before_value = if before_present { "Present" } else { "Not present" };
        let after_value = if after_present { "Kept" } else { "Removed" };
        let status = if selected && before_present && !after_present {
            "Removed"
        } else if keep_authenticity && key == "authenticity" {
            "Kept"
        } else {
            "Kept"
        };

        rows.push(json!({
            "field": label,
            "before": before_value,
            "after": after_value,
            "status": status
        }));
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_payload_strips_sensitive_fields() {
        let payload = json!({
            "report": {
                "privacy": [
                    {"label": "GPS coordinates", "severity": "High", "value": "40.7128, -74.0060"},
                    {"label": "Serial number", "severity": "Medium", "value": "ABC123"},
                    {"label": "Artist", "severity": "High", "value": "Jane Doe"}
                ],
                "authenticity": {"verdict": "Likely AI-generated", "confidence": 82},
                "image": {"filename": "photo.jpg"}
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

    #[test]
    fn test_builtin_explanation_contains_privacy_summary() {
        let payload = json!({
            "report": {
                "privacy": [{"label": "GPS coordinates", "severity": "High", "value": "41.0, -73.0"}],
                "authenticity": {"verdict": "Inconclusive", "confidence": 50}
            }
        });

        let explanation = build_builtin_explanation(&payload);
        assert_eq!(explanation.source, "Built-in explanation");
        assert!(explanation.summary.contains("Inconclusive"));
    }
}
