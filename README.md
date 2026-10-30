#!/usr/bin/env python3
import os
from pathlib import Path

from PIL import Image, ImageDraw
import piexif

ROOT = Path(__file__).resolve().parent
OUT_DIR = ROOT / "generated"
OUT_DIR.mkdir(exist_ok=True)


def make_fake_exif_photo(path: Path):
    img = Image.new("RGB", (1200, 900), color=(230, 225, 220))
    draw = ImageDraw.Draw(img)
    draw.rectangle((150, 220, 1050, 700), fill=(245, 240, 234), outline=(80, 80, 80), width=3)
    draw.text((250, 330), "Sample photo with EXIF", fill=(30, 30, 30), font=None)

    exif_dict = {
        "0th": {
            piexif.ImageIFD.Make: "Acme Camera",
            piexif.ImageIFD.Model: "Model X9",
            piexif.ImageIFD.Software: "Adobe Photoshop 25.1",
            piexif.ImageIFD.Artist: "A. Person",
            piexif.ImageIFD.Copyright: "Copyright A. Person",
            piexif.ImageIFD.DateTime: "2024:03:02 10:14:31",
            piexif.ImageIFD.OriginalDate: "2024:03:02 10:14:31",
            piexif.ImageIFD.DigitalizedDate: "2024:03:02 10:14:31",
        },
        "GPS": {
            piexif.GPSIFD.GPSLatitudeRef: "N",
            piexif.GPSIFD.GPSLatitude: ((40, 1), (45, 1), (12.5, 1)),
            piexif.GPSIFD.GPSLongitudeRef: "W",
            piexif.GPSIFD.GPSLongitude: ((73, 1), (59, 1), (21.4, 1)),
        },
    }
    exif_bytes = piexif.dump(exif_dict)
    img.save(path, "JPEG", quality=90, exif=exif_bytes)


def make_plain_photo(path: Path):
    img = Image.new("RGB", (1200, 900), color=(242, 242, 238))
    draw = ImageDraw.Draw(img)
    draw.rectangle((180, 220, 1040, 700), fill=(250, 248, 244), outline=(90, 90, 90), width=2)
    draw.text((270, 360), "Plain sample photo", fill=(18, 18, 18))
    img.save(path, "JPEG", quality=90)


def make_low_quality_photo(path: Path):
    src = OUT_DIR / "plain_photo.jpg"
    img = Image.open(src)
    img.save(path, "JPEG", quality=35, optimize=True)


if __name__ == "__main__":
    make_fake_exif_photo(OUT_DIR / "fake_exif_photo.jpg")
    make_plain_photo(OUT_DIR / "plain_photo.jpg")
    make_low_quality_photo(OUT_DIR / "low_quality_photo.jpg")
    print("Sample images generated in:", OUT_DIR)
    print("- fake_exif_photo.jpg: image with GPS/device EXIF")
    print("- plain_photo.jpg: clean baseline image")
    print("- low_quality_photo.jpg: low-quality JPEG")
    print("Drop in your own AI or synthetic sample to test authenticity on a real local file.")

