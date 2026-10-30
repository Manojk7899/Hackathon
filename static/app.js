* { box-sizing: border-box; }

:root {
  --paper: #F4F1EA;
  --ink: #1B1B19;
  --line: #c9c0b5;
  --soft: #f3efe9;
  --signal: #D9441F;
  --panel: #f9f6f2;
  --muted: rgba(27, 27, 25, 0.75);
}

html, body {
  margin: 0;
  padding: 0;
  background: var(--paper);
  color: var(--ink);
  font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  line-height: 1.5;
}

body {
  min-height: 100vh;
}

h1, h2 {
  font-family: Georgia, "Times New Roman", serif;
  font-weight: 600;
  letter-spacing: 0.02em;
}

h1 {
  margin: 0;
  font-size: clamp(2rem, 3vw, 2.7rem);
}

h2 {
  margin: 0;
  font-size: 1.08rem;
}

.page-shell {
  max-width: 1280px;
  margin: 0 auto;
  padding: 20px 18px 24px;
}

.topbar {
  padding: 12px 0 18px;
  border-bottom: 1px solid var(--line);
}

.tagline {
  margin-top: 4px;
  font-family: ui-monospace, "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
  font-size: 0.7rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.main-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.05fr) minmax(0, 1.3fr);
  gap: 18px;
  padding-top: 18px;
}

.panel {
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.08);
  padding: 14px;
}

.section-header {
  border-bottom: 1px solid var(--line);
  padding-bottom: 8px;
  margin-bottom: 12px;
}

.upload-box {
  position: relative;
  border: 1px dashed var(--line);
  min-height: 120px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  background: var(--soft);
  cursor: pointer;
}

.upload-box:focus-within,
.upload-box:hover {
  border-color: var(--signal);
}

.upload-box input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.upload-box label {
  cursor: pointer;
  font-weight: 600;
}

.helper,
.helper-inline,
.auth-note {
  font-size: 0.8rem;
  color: var(--muted);
}

.image-stage {
  position: relative;
  margin-top: 14px;
  border: 1px solid var(--line);
  min-height: 320px;
  background: #f8f6f3;
  display: flex;
  align-items: center;
  justify-content: center;
}

#preview-image {
  width: 100%;
  max-height: 70vh;
  display: block;
  object-fit: contain;
}

.overlay-layer {
  position: absolute;
  inset: 0;
}

.region-box {
  position: absolute;
  border: 1px solid var(--signal);
  background: rgba(217, 68, 31, 0.12);
  cursor: pointer;
}

.region-box.selected {
  background: rgba(217, 68, 31, 0.2);
  box-shadow: inset 0 0 0 1px var(--signal);
}

.analysis-status {
  margin-top: 10px;
  font-family: ui-monospace, "SFMono-Regular", Consolas, monospace;
  font-size: 0.72rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--signal);
}

.overlay-toggle {
  margin-top: 8px;
}

.empty-state {
  padding: 10px 4px;
  color: var(--muted);
  border-top: 1px solid var(--line);
}

.report-table,
.verification-table {
  width: 100%;
  border-collapse: collapse;
  border-top: 1px solid var(--line);
  font-size: 0.92rem;
}

.report-table th,
.report-table td,
.verification-table th,
.verification-table td {
  padding: 10px 6px;
  border-bottom: 1px solid var(--line);
  text-align: left;
  vertical-align: top;
  font-family: ui-monospace, "SFMono-Regular", Consolas, monospace;
}

.report-table th,
.verification-table th {
  font-family: Georgia, "Times New Roman", serif;
  font-weight: 600;
  letter-spacing: 0.05em;
  font-size: 0.72rem;
  text-transform: uppercase;
}

.severity-tag {
  display: inline-block;
  min-width: 62px;
  border: 1px solid var(--ink);
  padding: 2px 0;
  text-align: center;
  font-size: 0.72rem;
  letter-spacing: 0.08em;
  font-family: ui-monospace, monospace;
}

.severity-tag.high {
  border-color: var(--signal);
  color: var(--signal);
}

.verdict-row {
  font-size: 1.12rem;
  font-weight: 600;
  margin-bottom: 6px;
}

.confidence-row {
  margin-top: 10px;
}

.confidence-line {
  position: relative;
  height: 2px;
  background: var(--line);
  overflow: hidden;
}

.confidence-line span {
  display: block;
  width: 0;
  height: 100%;
  background: var(--signal);
}

.confidence-label {
  margin-top: 8px;
  font-family: ui-monospace, monospace;
  font-size: 0.8rem;
}

.reasons-list,
.explanation-risks,
.explanation-next {
  margin: 10px 0 0;
  padding-left: 1.2rem;
  font-size: 0.9rem;
}

.explain-box {
  margin-top: 18px;
}

.explanation-panel {
  border: 1px solid var(--line);
  padding: 10px;
  background: rgba(255,255,255,0.08);
}

.explanation-label {
  font-family: ui-monospace, monospace;
  font-size: 0.68rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  margin-bottom: 8px;
}

.explain-actions {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 12px;
}

.checkbox-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px 10px;
  margin-bottom: 10px;
}

.checkbox-grid label,
.toggle-row,
.blur-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.92rem;
}

.blur-row {
  margin-top: 12px;
}

.select, select {
  border: 1px solid var(--line);
  background: var(--paper);
  padding: 7px 8px;
  color: var(--ink);
  font: inherit;
}

button {
  border: 1px solid var(--ink);
  background: var(--paper);
  color: var(--ink);
  padding: 10px 14px;
  cursor: pointer;
  font: inherit;
}

button.primary-button {
  background: var(--signal);
  border-color: var(--signal);
  color: #fff;
}

button.secondary-button {
  background: transparent;
}

button:focus-visible,
input:focus-visible,
select:focus-visible {
  outline: 2px solid var(--signal);
  outline-offset: 2px;
}

.site-footer {
  margin-top: 18px;
  padding-top: 12px;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 0.78rem;
}

.hidden { display: none !important; }

@media (max-width: 900px) {
  .main-layout {
    grid-template-columns: 1fr;
  }
}
