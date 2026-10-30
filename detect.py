import io
from typing import Any, Dict, List, Tuple

import cv2
import numpy as np
from PIL import Image

CASCADE_PATH = cv2.data.haarcascades + "haarcascade_frontalface_default.xml"
PLATE_PATH = cv2.data.haarcascades + "haarcascade_russian_plate_number.xml"


def detect_faces_and_plates(image_rgb: np.ndarray) -> Tuple[List[Dict[str, Any]], List[Dict[str, Any]]]:
    gray = cv2.cvtColor(image_rgb, cv2.COLOR_RGB2GRAY)
    faces: List[Dict[str, Any]] = []
    plates: List[Dict[str, Any]] = []

    face_cascade = cv2.CascadeClassifier(CASCADE_PATH)
    if not face_cascade.empty():
        detected_faces = face_cascade.detectMultiScale(gray, scaleFactor=1.1, minNeighbors=5, minSize=(30, 30))
        for x, y, w, h in detected_faces:
            faces.append({"x": int(x), "y": int(y), "w": int(w), "h": int(h), "label": "face"})

    plate_cascade = cv2.CascadeClassifier(PLATE_PATH)
    if not plate_cascade.empty():
        detected_plates = plate_cascade.detectMultiScale(gray, scaleFactor=1.2, minNeighbors=3, minSize=(20, 10))
        for x, y, w, h in detected_plates:
            plates.append({"x": int(x), "y": int(y), "w": int(w), "h": int(h), "label": "plate"})

    return faces, plates


def _blur_crop(base_image: np.ndarray, region: Dict[str, Any], blur_mode: str = "gaussian") -> np.ndarray:
    x = int(region.get("x", 0))
    y = int(region.get("y", 0))
    w = int(region.get("w", 10))
    h = int(region.get("h", 10))
    x1 = max(0, x)
    y1 = max(0, y)
    x2 = min(base_image.shape[1], x + w)
    y2 = min(base_image.shape[0], y + h)
    crop = base_image[y1:y2, x1:x2].copy()

    if crop.size == 0:
        return base_image

    if blur_mode == "pixelate":
        block = 16
        small = cv2.resize(crop, (max(1, crop.shape[1] // block), max(1, crop.shape[0] // block)), interpolation=cv2.INTER_LINEAR)
        blurred = cv2.resize(small, (crop.shape[1], crop.shape[0]), interpolation=cv2.INTER_NEAREST)
    else:
        blurred = cv2.GaussianBlur(crop, (35, 35), 0)

    base_image[y1:y2, x1:x2] = blurred
    return base_image


def apply_region_blur(image: Image.Image, regions: List[Dict[str, Any]], blur_mode: str = "gaussian") -> Image.Image:
    rgb = np.array(image.convert("RGB"))
    for region in regions:
        if region.get("kind") in {"face", "plate"} or region.get("label") in {"face", "plate"}:
            rgb = _blur_crop(rgb, region, blur_mode=blur_mode)
    return Image.fromarray(rgb.astype("uint8"))


__all__ = ["detect_faces_and_plates", "apply_region_blur"]
