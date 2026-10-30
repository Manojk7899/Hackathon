import os
from pathlib import Path

from PIL import Image, ImageDraw
import piexif

ROOT = Path(__file__).resolve().parent
ROOT.mkdir(exist_ok=True)


def save_fake_gps_example(path: Path):
    img = Image.new('RGB', (800, 600), color=(223, 220, 212))
    draw = ImageDraw.Draw(img)
    draw.rectangle((120, 120, 680, 480), fill=(245, 240, 232))
    draw.text((140, 180), 'GPS and device metadata sample', fill=(20, 20, 20))

    exif_ifd = {
        piexif.ImageIFD.Make: 'Example Labs',
        piexif.ImageIFD.Model: 'LensPro X1',
        piexif.ImageIFD.Software: 'Example Editor 4.2',
        piexif.ImageIFD.Artist: 'A. Researcher',
        piexif.ImageIFD.Copyright: 'Copyright A. Researcher',
        piexif.ImageIFD.DateTime: '2024:04:17 12:35:44',
    }
    gps_ifd = {
        piexif.GPSIFD.GPSLatitudeRef: 'N',
        piexif.GPSIFD.GPSLatitude: ((40, 1), (42, 1), (32, 100)),
        piexif.GPSIFD.GPSLongitudeRef: 'W',
        piexif.GPSIFD.GPSLongitude: ((74, 1), (0, 1), (25, 100)),
    }
    exif_bytes = piexif.dump({"0th": exif_ifd, "GPS": gps_ifd})
    img.save(path, exif=exif_bytes)


def save_plain_example(path: Path):
    img = Image.new('RGB', (900, 600), color=(245, 239, 230))
    draw = ImageDraw.Draw(img)
    draw.rectangle((150, 150, 750, 450), outline=(35, 35, 35), width=3)
    draw.text((220, 260), 'Plain photo sample', fill=(30, 30, 30))
    img.save(path)


def save_low_quality_example(path: Path):
    img = Image.new('RGB', (640, 480), color=(230, 220, 208))
    draw = ImageDraw.Draw(img)
    for i in range(10):
        x = 40 + i * 55
        draw.rectangle((x, 60, x + 30, 420), fill=(70 + i * 5, 80 + i * 4, 95 + i * 2))
    draw.text((70, 200), 'Low-quality JPEG sample', fill=(30, 30, 30))
    img.save(path, quality=30, optimize=True)


if __name__ == '__main__':
    save_fake_gps_example(ROOT / 'sample_fake_gps.jpg')
    save_plain_example(ROOT / 'sample_plain.png')
    save_low_quality_example(ROOT / 'sample_low_quality.jpg')
    print('Generated sample images in', ROOT)


