# Rust build for TruthVision

This project is a Rust version of the TruthVision local-first image privacy tool.

## Run

```bash
cargo run
```

Then open:

http://localhost:8000

## Notes

- The frontend is plain HTML, CSS, and JavaScript.
- The backend is Rust with Axum.
- The app is intentionally honest: model-based authenticity is heuristic and can be unavailable.
- This is a working local prototype, not a forensic-grade detector.
