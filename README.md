# TruthVision Lens

TruthVision Lens is a local-first photo privacy and authenticity checker built for quick, honest inspection.

## What it does

- Upload JPEG or PNG images up to 15 MB.
- Extract and summarize privacy leaks from EXIF, PNG text chunks, and embedded metadata.
- Estimate authenticity using metadata and heuristic signals.
- Let the user remove selected metadata and blur marked regions.
- Re-scan the cleaned file and compare before/after.
- Optionally explain the results using a sanitized text summary sent to Gemini.

## Run

1. Install Rust and Cargo.
2. Copy `.env.example` to `.env` and add a Gemini key if you want explanation calls.
3. Run:

```bash
cargo run
```

Then open http://localhost:8000

## Project notes

- The app serves a plain HTML, CSS, and JavaScript frontend from the static directory.
- Image handling is local-only. No analytics or image storage are used.
- Authenticity verdicts are heuristic and intentionally conservative. The app flags when a model is unavailable.

## Evaluation

| Item | Status |
| --- | --- |
| Measured accuracy | TODO |
| Dataset used | TODO |
| Notes | TODO |

## Known limitations

- Detection drops on unseen generators and heavily compressed images.
- Missing metadata is a weak signal only.
- Metadata removal does not remove visible identifiers or sensor fingerprints such as PRNU.
- Face and plate detection are basic and can miss cases.
- LLM explanations can be wrong or overconfident and are not forensic results.

## What existed before the hackathon

Placeholder: this project was scaffolded as a local-first privacy check tool and transformed into the TruthVision Lens prototype for this hackathon.

## Demo script

1. Open the app and upload a photo with a location or device EXIF tag.
2. Review the report and authenticity verdict.
3. Select metadata groups to remove and run the clean step.
4. Review the before/after verification table.
5. Click Explain these results to generate a plain-language explanation from the sanitized summary.
6. Note the limitations: compressed files, missing metadata, and heuristic authenticity cues.

## Samples

Use the helper in `samples/generate_samples.py` to create three test images:

- a photo with fake GPS and device metadata,
- a plain photo,
- a JPEG that has been re-saved at low quality.

Do not invent AI-generated samples; add your own if needed.
