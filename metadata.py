from __future__ import annotations

import io
import os
import re
from typing import Any, Dict, List

from PIL import Image
import piexif


EXIF_TAG_MAP = {
    "gps": ["GPS GPSLatitude", "GPS GPSLongitude", "GPS GPSLatitudeRef", "GPS GPSLongitudeRef"],
    "device": ["0th 271", "0th 272", "0th 306", "0th 315"],
    "owner": ["0th 315", "0th 33432", "0th 0x0009"],
    "timestamps": ["0th 306", "0th 36867", "0th 36868", "GPS GPSDateStamp", "GPS GPSTimeStamp"],
    "thumbnail": ["1st 513"],
}


def _clean_exif_value(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray)):
        try:
            return value.decode("utf-8", errors="replace")
        except Exception:
            return str(value)
    if isinstance(value, tuple):
        return [str(v) for v in value]
    return value


def _decimal_from_dms(dms_value: Any, ref: Any) -> float:
    if isinstance(dms_value, tuple) and len(dms_value) == 3:
        deg, minute, sec = dms_value
        try:
            value = float(deg) + (float(minute) / 60.0) + (float(sec) / 3600.0)
        except (TypeError, ValueError):
            return 0.0
        if ref in ("S", "W"):
            value *= -1
        return value
    if isinstance(dms_value, (int, float)):
        return float(dms_value)
    return 0.0


def _extract_gps_from_exif(exif_dict: Dict[str, Any]) -> Dict[str, Any]:
    gps_data = exif_dict.get("GPS", {})
    if not gps_data:
        return {}
    lat = gps_data.get(2)
    lat_ref = gps_data.get(1)
    lon = gps_data.get(4)
    lon_ref = gps_data.get(3)
    if not lat or not lon:
        return {}
    try:
        lat_dec = _decimal_from_dms(lat, lat_ref)
        lon_dec = _decimal_from_dms(lon, lon_ref)
    except Exception:
        return {}
    return {
        "latitude": round(lat_dec, 6),
        "longitude": round(lon_dec, 6),
        "coordinates": {"lat": round(lat_dec, 6), "lng": round(lon_dec, 6)},
    }


def _find_generator_strings(meta_fields: Dict[str, Any]) -> List[str]:
    markers = [
        "Adobe Photoshop",
        "Lightroom",
        "Photoshop",
        "Canva",
        "Midjourney",
        "Stable Diffusion",
        "DALL-E",
        "Generative Fill",
        "Adobe Firefly",
        "OpenAI",
        "TensorFlow",
        "AI",
        "Camera Raw",
        "Aperture",
        "GIMP",
    ]
    found: List[str] = []
    for key, value in meta_fields.items():
        if not isinstance(value, str):
            continue
        lowered = value.lower()
        for marker in markers:
            if marker.lower() in lowered:
                found.append(f"{key}: {value}")
                break
    return found


def extract_metadata(file_bytes: bytes, filename: str) -> Dict[str, Any]:
    metadata: Dict[str, Any] = {
        "filename": filename,
        "gps": {},
        "device": {},
        "timestamps": {},
        "owner": {},
        "thumbnail_present": False,
        "software": None,
        "generator_strings": [],
        "c2pa_marker": False,
        "has_png_text": False,
        "all_fields": {},
        "format": None,
        "findings": [],
    }

    try:
        image = Image.open(io.BytesIO(file_bytes))
        metadata["format"] = image.format or "UNKNOWN"
    except Exception:
        return metadata

    try:
        exif_data = piexif.load(image.info.get("exif", b"")) if image.info.get("exif") else {}
    except Exception:
        exif_data = {}

    if exif_data:
        ifd0 = exif_data.get("0th", {})
        ifd1 = exif_data.get("1st", {})
        gps_data = exif_data.get("GPS", {})
        metadata["all_fields"].update({**ifd0, **gps_data, **ifd1})

        if any(v for v in ifd1.values()):
            metadata["thumbnail_present"] = True
            metadata["findings"].append({"label": "Embedded thumbnail", "value": "Yes", "severity": "Medium"})

        software = ifd0.get(305)
        if software:
            metadata["software"] = _clean_exif_value(software)
            metadata["findings"].append({"label": "Software tag", "value": metadata["software"], "severity": "Medium"})

        owner_values = {
            "Artist": ifd0.get(315),
            "Author": ifd0.get(0x0009),
            "Copyright": ifd0.get(33432),
            "XPAuthor": ifd0.get(0x0009),
        }
        for key, value in owner_values.items():
            if value:
                cleaned = _clean_exif_value(value)
                metadata["owner"][key] = cleaned
                metadata["findings"].append({"label": key, "value": cleaned, "severity": "High"})

        device_values = {
            "Make": ifd0.get(271),
            "Model": ifd0.get(272),
            "Serial Number": ifd0.get(0x000A) if 0x000A in ifd0 else None,
        }
        for key, value in device_values.items():
            if value:
                cleaned = _clean_exif_value(value)
                metadata["device"][key] = cleaned
                metadata["findings"].append({"label": key, "value": cleaned, "severity": "Medium"})

        timestamps = {
            "DateTimeOriginal": ifd0.get(36867),
            "DateTimeDigitized": ifd0.get(36868),
            "DateTime": ifd0.get(306),
        }
        for key, value in timestamps.items():
            if value:
                cleaned = _clean_exif_value(value)
                metadata["timestamps"][key] = cleaned
                metadata["findings"].append({"label": key, "value": cleaned, "severity": "Medium"})

        gps = _extract_gps_from_exif(exif_data)
        if gps:
            metadata["gps"] = gps
            metadata["findings"].append({
                "label": "GPS coordinates",
                "value": f"{gps['latitude']:.6f}, {gps['longitude']:.6f}",
                "severity": "High",
            })

    if metadata["format"] == "PNG":
        png_text = getattr(image, "text", {}) or {}
        if png_text:
            metadata["has_png_text"] = True
            metadata["all_fields"].update(png_text)
            for key, value in png_text.items():
                if isinstance(value, str) and key.lower() in {"software", "comment", "artist", "author", "copyright", "creation_time", "datetime"}:
                    metadata["generator_strings"].append(f"{key}: {value}")
                    metadata["findings"].append({
                        "label": key,
                        "value": value,
                        "severity": "High" if key.lower() in {"artist", "author", "copyright"} else "Medium",
                    })

    generator_hits = _find_generator_strings(metadata["all_fields"])
    if generator_hits:
        metadata["generator_strings"].extend(generator_hits)
        metadata["findings"].append({
            "label": "Generator or editing metadata",
            "value": "; ".join(generator_hits[:3]),
            "severity": "Medium",
        })

    lower_bytes = file_bytes.lower()
    if b"c2pa" in lower_bytes or b"jumbf" in lower_bytes or b"c2pa" in lower_bytes:
        metadata["c2pa_marker"] = True
        metadata["findings"].append({"label": "C2PA marker detected", "value": "Marker present", "severity": "Medium"})

    metadata["generator_strings"] = list(dict.fromkeys(metadata["generator_strings"]))
    return metadata


def _gather_selected_fields(selected_fields: List[str]) -> List[str]:
    mapping = {
        "location": ["gps"],
        "device": ["device"],
        "timestamps": ["timestamps"],
        "owner": ["owner"],
        "thumbnail": ["thumbnail_present"],
        "everything_else": ["software", "generator_strings", "c2pa_marker"],
        "authenticity": ["software", "generator_strings"],
    }
    result: List[str] = []
    for field in selected_fields:
        result.extend(mapping.get(field, [field]))
    return result


def apply_metadata_cleanup(file_bytes: bytes, selected_fields: List[str], keep_authenticity_fields: bool, file_name: str):
    image = Image.open(io.BytesIO(file_bytes))
    original = extract_metadata(file_bytes, file_name)
    processed = file_bytes

    selected = set(selected_fields)

    if image.format == "JPEG":
        exif = piexif.load(image.info.get("exif", b"")) if image.info.get("exif") else {"0th": {}, "Exif": {}, "GPS": {}, "1st": {}}
        if "location" in selected:
            exif["GPS"] = {}
        if "device" in selected:
            exif["0th"][271] = b""
            exif["0th"][272] = b""
            exif["0th"][0x000A] = b""
        if "timestamps" in selected:
            exif["0th"][306] = b""
            exif["0th"][36867] = b""
            exif["0th"][36868] = b""
        if "owner" in selected:
            exif["0th"][315] = b""
            exif["0th"][33432] = b""
        if "thumbnail" in selected:
            exif["1st"] = {}
        if "everything_else" in selected:
            exif["0th"][305] = b""
            exif["0th"][34665] = b""
        if keep_authenticity_fields and 305 in exif["0th"]:
            exif["0th"][305] = exif["0th"].get(305)

        rgb = image.convert("RGB")
        buffer = io.BytesIO()
        rgb.save(buffer, format="JPEG", quality=95, exif=piexif.dump(exif))
        processed = buffer.getvalue()

    elif image.format == "PNG":
        if any(group in selected for group in ["location", "device", "timestamps", "owner", "thumbnail", "everything_else", "authenticity"]):
            rgb = image.convert("RGB")
            buffer = io.BytesIO()
            rgb.save(buffer, format="PNG")
            processed = buffer.getvalue()

    cleaned = extract_metadata(processed, file_name)
    return processed, cleaned


def verify_cleanup(before: Dict[str, Any], after: Dict[str, Any], selection: List[str], keep_authenticity_fields: bool) -> List[Dict[str, Any]]:
    rows: List[Dict[str, Any]] = []
    fields = {
        "gps": "GPS coordinates",
        "device": "Device details",
        "timestamps": "Timestamps",
        "owner": "Owner fields",
        "thumbnail_present": "Embedded thumbnail",
        "software": "Software tag",
        "generator_strings": "Generator metadata",
    }

    for key, label in fields.items():
        before_value = before.get(key)
        after_value = after.get(key)
        if before_value in ({}, None, [], "") and after_value in ({}, None, [], "") and key not in selection:
            continue

        if key in selection or (key == "software" and keep_authenticity_fields):
            status = "Removed" if before_value not in ({}, None, []) and after_value in ({}, None, []) else "Kept"
        else:
            status = "Kept"

        rows.append({
            "field": label,
            "before": "Not present" if before_value in ({}, None, []) else str(before_value),
            "after": "Removed" if after_value in ({}, None, []) else str(after_value),
            "status": status,
        })

    return rows


__all__ = [
    "extract_metadata",
    "apply_metadata_cleanup",
    "verify_cleanup",
]


path = "";


"""
Legacy placeholder kept for compatibility.
"""


if __name__ == "__main__":
    print("metadata utilities loaded")
    
    
    
    
    
    
    
    
    
    
    
    
    """
    """

	
























"""
