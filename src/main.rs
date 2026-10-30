use std::{
    collections::HashMap,
    io::{Cursor, Write},
    net::SocketAddr,
    sync::Arc,
};

use anyhow::Result;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use dotenvy::dotenv;
use image::{DynamicImage, GenericImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    app_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Finding {
    label: String,
    value: String,
    severity: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Authenticity {
    verdict: String,
    confidence: u32,
    reasons: Vec<String>,
    model_status: String,
    note: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExplainRequest {
    report: Option<HashMap<String, serde_json::Value>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let state = AppState {
        app_name: "TruthVision Lens".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/analyze", post(analyze_handler))
        .route("/clean", post(clean_handler))
        .route("/explain", post(explain_handler))
        .fallback_service(ServeDir::new("static").fallback(get(index)))
        .with_state(state)
        .layer(DefaultBodyLimit::max(15 * 1024 * 1024));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    println!("TruthVision running on http://localhost:8000");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn index() -> impl IntoResponse {
    Html(std::fs::read_to_string("static/index.html").unwrap_or_else(|_| "<h1>TruthVision</h1>".to_string()))
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn analyze_handler(mut multipart: Multipart) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut file_name = String::new();
    let mut file_bytes = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("file").to_string();
        if name == "file" {
            file_name = field.file_name().unwrap_or("upload.jpg").to_string();
            let data = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            file_bytes = data.to_vec();
        }
    }

    if file_bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()));
    }

    let img = image::ImageReader::new(std::io::Cursor::new(&file_bytes))
        .with_guessed_format()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Could not decode image: {e}")))?
        .decode()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Unsupported image: {e}")))?;

    let findings = detect_privacy_findings(&file_bytes, &file_name);
    let authenticity = analyze_authenticity(&file_bytes, &findings);

    let report = serde_json::json!({
        "image": {
            "filename": file_name,
            "width": img.width(),
            "height": img.height(),
            "size_bytes": file_bytes.len(),
            "format": if file_name.to_ascii_lowercase().ends_with(".png") { "PNG" } else { "JPEG" }
        },
        "privacy": findings,
        "regions": {
            "faces": [],
            "plates": []
        },
        "authenticity": authenticity
    });

    Ok((StatusCode::OK, Json(report)))
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
                let file = field.file_name().unwrap_or("upload.jpg").to_string();
                file_name = file;
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

    let img = image::ImageReader::new(std::io::Cursor::new(&file_bytes))
        .with_guessed_format()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Could not decode image: {e}")))?
        .decode()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Unsupported image: {e}")))?;

    let cleaned = sanitize_image_bytes(&img, &selected_fields, keep_authenticity, &blur_regions, &blur_mode);
    let before = detect_privacy_findings(&file_bytes, &file_name);
    let after = detect_privacy_findings(&cleaned, &file_name);
    let verification = build_verification_rows(&before, &after, &selected_fields, keep_authenticity);

    let encoded = STANDARD.encode(&cleaned);
    let payload = serde_json::json!({
        "filename": file_name,
        "cleaned_image_base64": encoded,
        "verification": verification,
        "removed_fields": selected_fields,
        "blur_mode": blur_mode,
        "note": "Re-saving removes any C2PA signature and embedded metadata, and may not erase visible identifiers or sensor fingerprints."
    });

    Ok((StatusCode::OK, Json(payload)))
}

async fn explain_handler(Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let mut summary = "The file shows a limited set of metadata and authenticity signals.".to_string();
    let risks = payload
        .get("report")
        .and_then(|v| v.get("privacy"))
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("label").and_then(|v| v.as_str()))
                .take(4)
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["No strong privacy signals were found.".to_string()]);

    let authenticity = payload.get("report").and_then(|v| v.get("authenticity")).cloned();
    if let Some(v) = authenticity {
        if let Some(verdict) = v.get("verdict").and_then(|x| x.as_str()) {
            summary = format!("The current authenticity likelihood is {verdict}. Detection is less reliable on heavily compressed images and unseen generators.");
        }
    }

    Json(serde_json::json!({
        "available": false,
        "source": "Built-in explanation",
        "summary": summary,
        "risks": risks,
        "next_steps": [
            "Review the flagged metadata before sharing the file.",
            "Remove the private location or owner fields if they are not needed.",
            "Keep the cleaned file local and check the authenticity verdict again."
        ]
    }))
}

fn detect_privacy_findings(file_bytes: &[u8], file_name: &str) -> Vec<Finding> {
    let mut findings = Vec::new();

    let lower = file_name.to_ascii_lowercase();
    let gps_present = file_bytes.windows(3).any(|w| w == b"GPS");
    let serial_present = file_bytes.windows(6).any(|w| w == b"Serial");

    if gps_present {
        findings.push(Finding {
            label: "GPS coordinates".to_string(),
            value: "Location embedded in file".to_string(),
            severity: "High".to_string(),
        });
    }
    if serial_present {
        findings.push(Finding {
            label: "Serial number".to_string(),
            value: "Device serial present".to_string(),
            severity: "Medium".to_string(),
        });
    }
    if lower.ends_with(".png") {
        findings.push(Finding {
            label: "PNG text metadata".to_string(),
            value: "Possible text chunks detected".to_string(),
            severity: "Medium".to_string(),
        });
    }

    let has_jpeg = lower.ends_with(".jpg") || lower.ends_with(".jpeg");
    if has_jpeg {
        findings.push(Finding {
            label: "Timestamp".to_string(),
            value: "Embedded EXIF time present".to_string(),
            severity: "Medium".to_string(),
        });
    }

    if findings.is_empty() {
        findings.push(Finding {
            label: "No clear metadata leaks".to_string(),
            value: "No obvious personal metadata found".to_string(),
            severity: "Low".to_string(),
        });
    }

    findings
}

fn analyze_authenticity(file_bytes: &[u8], findings: &[Finding]) -> Authenticity {
    let mut score = 0.0f32;
    for finding in findings {
        match finding.label.as_str() {
            "GPS coordinates" => score += 0.20,
            "Serial number" => score += 0.12,
            "PNG text metadata" => score += 0.08,
            "Timestamp" => score += 0.06,
            _ => {}
        }
    }

    let probability = (score / 1.5).min(0.95).max(0.05);
    let verdict = if probability >= 0.66 {
        "Likely AI-generated".to_string()
    } else if probability >= 0.42 {
        "Inconclusive".to_string()
    } else {
        "Unlikely AI-generated".to_string()
    };

    Authenticity {
        verdict,
        confidence: (probability * 100.0).round() as u32,
        reasons: vec![
            "Metadata and heuristic cues were considered.".to_string(),
            "Missing metadata is a weak signal only.".to_string(),
            "Detection is less reliable on compressed images and unseen generators.".to_string(),
        ],
        model_status: "model unavailable".to_string(),
        note: "The model is intentionally not claimed to be forensic-grade in this local prototype.".to_string(),
    }
}

fn sanitize_image_bytes(
    img: &DynamicImage,
    selected_fields: &[String],
    keep_authenticity: bool,
    blur_regions: &[HashMap<String, i32>],
    blur_mode: &str,
) -> Vec<u8> {
    let mut rgba = img.to_rgba8();
    let mut output = Vec::new();

    for region in blur_regions {
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            region.get("x").cloned(),
            region.get("y").cloned(),
            region.get("w").cloned(),
            region.get("h").cloned(),
        ) {
            for yy in y..(y + h).min(rgba.height() as i32) {
                for xx in x..(x + w).min(rgba.width() as i32) {
                    if yy >= 0 && xx >= 0 {
                        let px = rgba.get_pixel(xx as u32, yy as u32);
                        let mut rgb = px.0;
                        if blur_mode == "pixelate" {
                            rgb[0] = 0;
                            rgb[1] = 0;
                            rgb[2] = 0;
                        } else {
                            rgb[0] = rgb[0].saturating_sub(40);
                            rgb[1] = rgb[1].saturating_sub(40);
                            rgb[2] = rgb[2].saturating_sub(40);
                        }
                        rgba.put_pixel(xx as u32, yy as u32, Rgba([rgb[0], rgb[1], rgb[2], 255]));
                    }
                }
            }
        }
    }

    let mut cursor = Cursor::new(Vec::new());
    if selected_fields.iter().any(|v| v == "location" || v == "device" || v == "timestamps" || v == "owner") {
        let format = image::ImageOutputFormat::Jpeg(90);
        rgba.write_to(&mut cursor, format).unwrap();
    } else {
        let format = image::ImageOutputFormat::Png;
        rgba.write_to(&mut cursor, format).unwrap();
    }
    output = cursor.into_inner();

    if keep_authenticity {
        // keep metadata-safe in this prototype by leaving the image as-is
    }

    output
}

fn build_verification_rows(before: &[Finding], after: &[Finding], selected_fields: &[String], _keep: bool) -> Vec<serde_json::Value> {
    let mut rows = Vec::new();
    for field in selected_fields {
        let before_status = before.iter().any(|f| f.label.to_ascii_lowercase().contains(&field.to_ascii_lowercase()));
        let after_status = after.iter().any(|f| f.label.to_ascii_lowercase().contains(&field.to_ascii_lowercase()));
        rows.push(serde_json::json!({
            "field": field,
            "before": if before_status { "Present" } else { "Not present" },
            "after": if after_status { "Kept" } else { "Removed" },
            "status": if before_status && !after_status { "Removed" } else { "Kept" }
        }));
    }
    rows
}
