# TruthVision Lens

TruthVision Lens is a local-first forensic image review and sanitization tool. It inspects an uploaded photo for embedded metadata leaks, face and license plate detection, authenticity signals, and optional Gemini-based plain-language explanation.

## What it is

The app answers two questions about a local photo:

1. Is it likely AI-generated or manipulated?
2. Does it leak personal data such as location, timestamps, device details, owner information, or thumbnails?

Everything runs locally on the device unless the user explicitly chooses the optional explanation feature, which sends only a sanitized text summary to Google's Gemini API.

## How to run

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python app.py
```

Then open:

http://localhost:8000

## Project structure

- `app.py` - FastAPI server and static hosting
- `metadata.py` - EXIF/XMP/PNG metadata inspection and selective stripping
- `detect.py` - face and plate detection plus region blurring
- `authenticity.py` - metadata and model-based authenticity analysis
- `explain.py` - sanitized explanation generation and fallback logic
- `static/index.html` - UI shell
- `static/styles.css` - offline forensic report styling
- `static/app.js` - upload, analysis, cleaning, and explanation workflow
- `samples/generate_samples.py` - creates three test assets
- `tests/` - automated validation for explanation behavior

## Privacy and safety rules

- No analytics, no tracking, and no filename or image logging.
- The backend sanitizes explanation payloads before sending anything to Gemini.
- GPS values, serial numbers, exact timestamps, owner names, file names, image pixels, and raw EXIF are never sent.
- Temporary files are removed immediately after processing.

## Known limitations

- Detection drops on unseen generators and heavy compression.
- Missing metadata is a weak signal.
- Metadata removal does not remove visible identifiers or camera sensor fingerprints (PRNU).
- Face and plate detection are basic and will miss some cases.
- LLM explanations can be wrong or overconfident and are not a forensic result.

## Evaluation

This project is a hackathon prototype. The following measurements are intentionally left as TODOs and should be filled in with measured results on a real dataset.

| Test / item | Result | Notes |
| --- | --- | --- |
| AI image detection on known generations | TODO | Record accuracy and sample count |
| Manipulated image detection on edited JPEGs | TODO | Record dataset and setup |
| Privacy leak detection on location-heavy EXIF | TODO | Measure recall and false positives |
| Plate detection reliability | TODO | Record precision and recall |
| Face detection reliability | TODO | Record dataset and false positives |
| Metadata stripping verification | TODO | Verify before/after EXIF keys |

Dataset used: TODO

## What existed before the hackathon

Placeholder: before this effort, the repository was blank and the app was not yet implemented. This project was built as a rapid local-first prototype for a privacy-focused forensic workflow during the hackathon.

## Model behavior and fallbacks

The authenticity model tries the following order:

1. `umm-maybe/AI-image-detector`
2. `Organika/sdxl-detector`

If neither model loads, the app continues to function and reports the verdict using metadata-only signals while clearly noting that the model is unavailable.

## Sample generation

Run:

```bash
python samples/generate_samples.py
```

This creates:

- `samples/generated/fake_exif_photo.jpg` - Image with GPS and fake device metadata
- `samples/generated/plain_photo.jpg` - Clean baseline image
- `samples/generated/low_quality_photo.jpg` - Re-saved JPEG at low quality

Do not fabricate AI-generated samples; add your own if you want to test authenticity on synthetic output.

## Demo script (2 minutes)

1. Open the app and upload a phone photo with embedded GPS.
2. Review the metadata report and note the location, device, and timestamp findings.
3. Select the location and owner metadata, then clean the file and download it.
4. Compare the before/after verification table to confirm fields were removed.
5. Upload an AI-style sample or a known synthetic image from your own set.
6. Check the authenticity verdict and note the weak-signal warnings for compressed images and unseen generators.
7. Click the explanation button to review the plain-language summary and its caveats.

## Notes

This project prioritizes a working, honest, local-first application over a broad feature set. The interface is designed to read like a forensic report rather than a consumer SaaS dashboard.

## Environment and optional Gemini setup

Use `.env.example` as a template:

```bash
cp .env.example .env
```

Then add a Gemini API key if you want explanation generation to use the cloud model. If no key is configured, the app falls back to a built-in local explanation without sending any image or raw metadata.

## What works

- JPEG and PNG upload validation with a 15 MB limit.
- Privacy metadata extraction and report generation.
- Face and plate detection overlays.
- Selective cleaning and verification of EXIF fields.
- Authenticity verdict generation with model and metadata signals.
- Local explanation fallback and sanitized Gemini path.

## What is weak or stubbed

- Model performance is limited by CPU-only inference and generic pretrained detectors.
- Plate detection is experimental and low reliability.
- LLM explanations are advisory and can be overconfident.
- The app intentionally avoids claiming forensic certainty.

## License

This project is a local hackathon prototype for privacy-focused image review and is intended for non-production evaluation on local machines.

