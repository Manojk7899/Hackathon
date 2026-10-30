from fastapi import FastAPI, File, Form, HTTPException, UploadFile
from fastapi.responses import JSONResponse
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
from PIL import Image
import io
import json
import os
import numpy as np

from metadata import extract_metadata, apply_metadata_cleanup, verify_cleanup
from detect import detect_faces_and_plates, apply_region_blur
from authenticity import analyze_authenticity

app = FastAPI(title="TruthVision Lens")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

static_dir = os.path.join(os.path.dirname(__file__), "static")
app.mount("/static", StaticFiles(directory=static_dir), name="static")

@app.get("/")
def index():
    with open(os.path.join(static_dir, "index.html"), "r", encoding="utf-8") as f:
        return f.read()

@app.get("/health")
def health():
    return {"status": "ok"}

@app.post("/analyze")
async def analyze(file: UploadFile = File(...)):
    if not file.filename:
        raise HTTPException(status_code=400, detail="No file was uploaded.")

    content = await file.read()
    if len(content) > 15 * 1024 * 1024:
        raise HTTPException(status_code=400, detail="File exceeds 15 MB limit.")

    try:
        image = Image.open(io.BytesIO(content))
        image.verify()
    except Exception as exc:
        raise HTTPException(status_code=400, detail=f"Unsupported or unreadable image: {exc}") from exc

    try:
        image = Image.open(io.BytesIO(content)).convert("RGB")
    except Exception as exc:
        raise HTTPException(status_code=400, detail=f"Could not decode image: {exc}") from exc

    metadata = extract_metadata(content, file.filename)
    faces, plates = detect_faces_and_plates(np.array(image))

    authenticity = analyze_authenticity(content, metadata)

    report = {
        "image": {
            "filename": file.filename,
            "width": image.width,
            "height": image.height,
            "format": image.format or "unknown",
            "size_bytes": len(content),
        },
        "privacy": metadata.get("findings", []),
        "regions": {
            "faces": faces,
            "plates": plates,
        },
        "authenticity": authenticity,
    }

    return JSONResponse(content={"report": report})


@app.post("/clean")
async def clean(
    file: UploadFile = File(...),
    selected_fields: str = Form(default="[]"),
    keep_authenticity_fields: str = Form(default="false"),
    blur_regions: str = Form(default="[]"),
    blur_mode: str = Form(default="gaussian"),
):
    if not file.filename:
        raise HTTPException(status_code=400, detail="No file was uploaded.")

    content = await file.read()
    if len(content) > 15 * 1024 * 1024:
        raise HTTPException(status_code=400, detail="File exceeds 15 MB limit.")

    try:
        data = json.loads(selected_fields)
    except json.JSONDecodeError:
        data = []

    try:
        region_data = json.loads(blur_regions)
    except json.JSONDecodeError:
        region_data = []

    keep_authenticity = str(keep_authenticity_fields).lower() == "true"
    blur_mode = blur_mode if blur_mode in {"gaussian", "pixelate"} else "gaussian"

    try:
        image = Image.open(io.BytesIO(content)).convert("RGB")
    except Exception as exc:
        raise HTTPException(status_code=400, detail=f"Could not decode image: {exc}") from exc

    original_meta = extract_metadata(content, file.filename)
    cleaned_bytes, cleaned_meta = apply_metadata_cleanup(
        content,
        selected_fields=data,
        keep_authenticity_fields=keep_authenticity,
        file_name=file.filename,
    )

    cleaned_image = Image.open(io.BytesIO(cleaned_bytes))
    cleaned_image = apply_region_blur(cleaned_image, region_data, blur_mode=blur_mode)
    output_buffer = io.BytesIO()
    cleaned_image.save(output_buffer, format=("PNG" if file.filename.lower().endswith(".png") else "JPEG"), quality=95)
    cleaned_image_bytes = output_buffer.getvalue()

    verification = verify_cleanup(original_meta, cleaned_meta, selection=data, keep_authenticity_fields=keep_authenticity)

    return JSONResponse(
        content={
            "filename": file.filename,
            "cleaned_image_base64": __import__("base64").b64encode(cleaned_image_bytes).decode("utf-8"),
            "verification": verification,
            "removed_fields": data,
            "blur_mode": blur_mode,
            "note": "Re-saving removes C2PA signatures and embedded metadata, and may not erase visible identifiers or sensor fingerprints.",
        }
    )


app.mount("/", StaticFiles(directory=static_dir, html=True), name="root")

if __name__ == "__main__":
    import uvicorn

    uvicorn.run("app:app", host="0.0.0.0", port=8000, reload=False)

