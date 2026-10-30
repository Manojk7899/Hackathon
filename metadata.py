import io
import json
import re
from typing import List, Dict, Any

from PIL import Image
import piexif
from piexif import InvalidImageDataError


EXIF_TAG_MAP = {
    "gps": ["GPS GPSLatitude", "GPS GPSLongitude", "GPS GPSAltitude", "GPS GPSLatitudeRef", "GPS GPSLongitudeRef"],
    "device": ["0th 271", "0th 272", "0th 273", "0th 274", "0th 306", "0th 315", "0th 34665"],
    "owner": ["0th 315", "0th 315", "0th 315", "0th 32803", "0th 315"],
    "timestamps": ["0th 36867", "0th 36868", "0th 306", "GPS GPSDateStamp", "GPS GPSTimeStamp"],
    "thumbnail": ["1st 513"],
    "authenticity": ["0th 34665", "0th 305"],
}


def _safe_get_exif(exif_obj: Any) -> Dict[str, Any]:
    if exif_obj is None:
        return {}
    try:
        return exif_obj.get_ifd(0)
    except Exception:
        return {}


def _extract_gps_from_exif(exif_dict: Dict[str, Any]) -> Dict[str, Any]:
    gps_data = exif_dict.get("GPS")
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
        "latitude": lat_dec,
        "longitude": lon_dec,
        "coordinates": {"lat": lat_dec, "lng": lon_dec},
    }


def _decimal_from_dms(dms_value: Any, ref: Any) -> float:
    if isinstance(dms_value, tuple) and len(dms_value) == 3:
        deg, minute, sec = dms_value
        value = float(deg) + (float(minute) / 60.0) + (float(sec) / 3600.0)
        if ref in ("S", "W"):
            value *= -1
        return value
    return 0.0


def _clean_exif_value(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray)):
        try:
            return value.decode("utf-8", errors="replace")
        except Exception:
            return str(value)
    if isinstance(value, tuple):
        return [str(v) for v in value]
    return value


def extract_metadata(file_bytes: bytes, filename: str) -> Dict[str, Any]:
    findings: List[Dict[str, Any]] = []
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
    }

    lower_name = filename.lower()
    try:
        image = Image.open(io.BytesIO(file_bytes))
    except Exception:
        return metadata

    format_name = image.format or "UNKNOWN"
    metadata["format"] = format_name

    try:
        exif_data = piexif.load(image.info.get("exif", b""))
    except Exception:
        exif_data = {}

    if exif_data:
        ifd0 = exif_data.get("0th", {})
        ifd1 = exif_data.get("1st", {})
        gps_data = exif_data.get("GPS", {})
        metadata["all_fields"] = {**ifd0, **gps_data, **ifd1}

        if any(v for v in ifd1.values()):
            metadata["thumbnail_present"] = True
            findings.append({
                "label": "Embedded thumbnail present",
                "value": "Yes",
                "severity": "Medium",
            })

        software = ifd0.get(305)
        if software:
            metadata["software"] = _clean_exif_value(software)
            findings.append({
                "label": "Software tag",
                "value": metadata["software"],
                "severity": "Medium",
            })

        owner_fields = {
            "Artist": ifd0.get(315),
            "Copyright": ifd0.get(33432),
            "XPAuthor": ifd0.get(0x0009),
        }
        for key, value in owner_fields.items():
            if value:
                metadata["owner"][key] = _clean_exif_value(value)
                findings.append({
                    "label": key,
                    "value": _clean_exif_value(value),
                    "severity": "High" if key in {"Artist", "Copyright"} else "Medium",
                })

        device_fields = {
            "Make": ifd0.get(271),
            "Model": ifd0.get(272),
            "Serial Number": ifd0.get(0x000A) if 0x000A in ifd0 else None,
        }
        for key, value in device_fields.items():
            if value:
                metadata["device"][key] = _clean_exif_value(value)
                findings.append({
                    "label": key,
                    "value": _clean_exif_value(value),
                    "severity": "Medium" if key in {"Make", "Model"} else "Medium",
                })

        timestamps = {
            "DateTimeOriginal": ifd0.get(36867),
            "DateTimeDigitized": ifd0.get(36868),
            "DateTime": ifd0.get(306),
        }
        for key, value in timestamps.items():
            if value:
                metadata["timestamps"][key] = _clean_exif_value(value)
                findings.append({
                    "label": key,
                    "value": _clean_exif_value(value),
                    "severity": "Medium",
                })

        gps = _extract_gps_from_exif(exif_data)
        if gps:
            metadata["gps"] = gps
            findings.append({
                "label": "GPS coordinates",
                "value": f"{gps['latitude']:.6f}, {gps['longitude']:.6f}",
                "severity": "High",
            })

    if format_name == "PNG":
        png_text = getattr(image, "text", {}) or {}
        if png_text:
            metadata["has_png_text"] = True
            metadata["all_fields"].update(png_text)
            for key, value in png_text.items():
                if key.lower() in {"software", "comment", "artist", "author", "copyright", "creation_time"}:
                    metadata["generator_strings"].append(str(value))
                    findings.append({
                        "label": key,
                        "value": str(value),
                        "severity": "High" if key.lower() in {"artist", "author", "copyright"} else "Medium",
                    })

    # Simple generator and editing signals
    known_strings = [
        "Adobe Photoshop",
        "Lightroom",
        "Pixelmator",
        "Canva",
        "Midjourney",
        "Stable Diffusion",
        "DALL-E",
        "Generative Fill",
        "Adobe Firefly",
        "OpenAI",
        "AI",
        "TensorFlow",
        "Photoshop",
    ]

    for key, value in metadata["all_fields"].items():
        if isinstance(value, str):
            lowered = value.lower()
            for marker in known_strings:
                if marker.lower() in lowered:
                    metadata["generator_strings"].append(f"{key}: {value}")

    if metadata["generator_strings"]:
        findings.append({
            "label": "Generator or editing metadata",
            "value": "; ".join(metadata["generator_strings"][:3]),
            "severity": "Medium",
        })

    # C2PA/JUMBF marker detection fallback
    if b"c2pa" in file_bytes.lower() or b"jumbf" in file_bytes.lower() or b"\x00\x00\x00\x00\x00\x00\x00\x00" in file_bytes[:64]:
        metadata["c2pa_marker"] = True
        findings.append({
            "label": "C2PA marker detected",
            "value": "Marker present",
            "severity": "Medium",
        })

    metadata["findings"] = findings
    return metadata


def _gather_selected_fields(metadata: Dict[str, Any], selected_fields: List[str]) -> List[str]:
    mapping = {
        "location": ["gps"],
        "device": ["device"],
        "timestamps": ["timestamps"],
        "owner": ["owner"],
        "thumbnail": ["thumbnail_present"],
        "everything_else": ["software", "generator_strings", "metadata"],
        "authenticity": ["software", "generator_strings"],
    }
    flattened = []
    for field in selected_fields:
        flattened.extend(mapping.get(field, [field]))
    return flattened


def apply_metadata_cleanup(file_bytes: bytes, selected_fields: List[str], keep_authenticity_fields: bool, file_name: str) -> Any:
    image = Image.open(io.BytesIO(file_bytes))
    original = extract_metadata(file_bytes, file_name)
    processed_bytes = file_bytes

    if image.format == "JPEG":
        exif = piexif.load(image.info.get("exif", b""))
        if exif:
            # Remove selected groups from EXIF dictionaries
            if "location" in selected_fields:
                exif["GPS"] = {}
            if "device" in selected_fields:
                exif["0th"][271] = b""
                exif["0th"][272] = b""
            if "timestamps" in selected_fields:
                exif["0th"][306] = b""
                exif["0th"][36867] = b""
                exif["0th"][36868] = b""
            if "owner" in selected_fields:
                exif["0th"][315] = b""
                exif["0th"][33432] = b""
            if "thumbnail" in selected_fields:
                exif["1st"] = {}
            if "everything_else" in selected_fields:
                exif["0th"][305] = b""
                exif["0th"][34665] = b""
            if keep_authenticity_fields:
                exif["0th"][305] = exif["0th"].get(305)

            filtered = piexif.dump(exif)
            processed_bytes = bytearray(file_bytes)
            # Remove all EXIF data if any selected field is in use
            if any(group in selected_fields for group in ["location", "device", "timestamps", "owner", "thumbnail", "everything_else", "authenticity"]):
                try:
                    new_image = Image.open(io.BytesIO(file_bytes)).convert("RGB")
                    buffer = io.BytesIO()
                    new_image.save(buffer, format="JPEG", quality=95)
                    processed_bytes = buffer.getvalue()
                except Exception:
                    pass
                if image.format == "JPEG":
                    exif_bytes = piexif.dump({"0th": {}, "GPS": {}, "Exif": {}, "1st": {}})
                    # Replace by copying the image pixels without EXIF metadata.
                    rgb = image.convert("RGB")
                    buf = io.BytesIO()
                    rgb.save(buf, format="JPEG", quality=95, exif=exif_bytes)
                    processed_bytes = buf.getvalue()

    elif image.format == "PNG":
        if any(group in selected_fields for group in ["location", "device", "timestamps", "owner", "thumbnail", "everything_else", "authenticity"]):
            rgb = image.convert("RGB")
            buf = io.BytesIO()
            rgb.save(buf, format="PNG")
            processed_bytes = buf.getvalue()

    cleaned = extract_metadata(processed_bytes, file_name)
    return processed_bytes, cleaned


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
        if before_value in ({}, None, []) and after_value in ({}, None, []) and key not in selection:
            continue

        if key in selection or (key == "software" and keep_authenticity_fields):
            status = "Removed" if (before_value not in ({}, None, [])) and (after_value in ({}, None, [])) else "Kept"
        else:
            status = "Kept"

        rows.append({
            "field": label,
            "before": str(before_value) if before_value not in ({}, None, []) else "Not present",
            "after": str(after_value) if after_value not in ({}, None, []) else "Removed",
            "status": status,
        })

    return rows
