* { box-sizing: border-box; }

:root {
  --paper: #F4F1EA;
  --ink: #1B1B19;
  --line: #c7c0b6;
  --soft: #ece5dc;
  --signal: #D9441F;
  --panel: #f8f6f2;
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
  font-size: 1.1rem;
}

.page-shell {
  max-width: 1280px;
  margin: 0 auto;
  padding: 20px 18px 32px;
}

.topbar {
  padding: 14px 0 18px;
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
  grid-template-columns: 1.05fr 1.3fr;
  gap: 18px;
  padding-top: 18px;
}

.panel {
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.1);
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
  min-height: 110px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  background: var(--soft);
  cursor: pointer;
  transition: border-color 0.15s ease;
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

.helper {
  font-size: 0.8rem;
  color: rgba(27, 27, 25, 0.75);
}

.image-stage {
  position: relative;
  margin-top: 14px;
  border: 1px solid var(--line);
  min-height: 320px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f8f6f3;
}

#preview-image {
  width: 100%;
  max-height: 68vh;
  object-fit: contain;
  display: block;
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
  background: rgba(217, 68, 31, 0.22);
  box-shadow: inset 0 0 0 1px var(--signal);
}

.analysis-status {
  margin-top: 10px;
  font-family: ui-monospace, monospace;
  font-size: 0.8rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--signal);
}

.report-box,
.auth-box,
.clean-box,
.verification-box {
  margin-bottom: 18px;
}

.empty-state {
  padding: 12px 4px;
  color: rgba(27, 27, 25, 0.8);
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
  font-family: serif;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  font-size: 0.75rem;
}

.severity-tag {
  display: inline-block;
  min-width: 62px;
  text-align: center;
  padding: 2px 0;
  border: 1px solid var(--ink);
  font-size: 0.72rem;
  letter-spacing: 0.08em;
  font-family: ui-monospace, monospace;
}

.severity-tag.high { border-color: var(--signal); color: var(--signal); }
.severity-tag.medium { border-color: var(--ink); }
.severity-tag.low { border-color: var(--ink); }

.verdict-row {
  margin-bottom: 6px;
  font-size: 1.2rem;
  font-weight: 600;
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

.reasons-list {
  margin: 10px 0 0;
  padding-left: 1.2rem;
  font-size: 0.9rem;
}

.checkbox-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px 10px;
  margin-bottom: 10px;
}

.checkbox-grid label,
.toggle-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.92rem;
}

.blur-row {
  display: flex;
  align-items: center;
  gap: 12px;
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
  color: #fff;
  border-color: var(--signal);
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
  font-size: 0.8rem;
  color: rgba(27, 27, 25, 0.8);
}

.hidden {
  display: none !important;
}

@media (max-width: 900px) {
  .main-layout {
    grid-template-columns: 1fr;
  }
}

