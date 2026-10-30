const fileInput = document.getElementById('file-input');
const previewImage = document.getElementById('preview-image');
const imageStage = document.getElementById('image-stage');
const uploadBox = document.getElementById('upload-box');
const privacyTable = document.getElementById('privacy-table');
const reportEmpty = document.getElementById('report-empty');
const authenticityPanel = document.getElementById('authenticity-panel');
const verdictText = document.getElementById('verdict-text');
const confidenceValue = document.getElementById('confidence-value');
const confidenceTick = document.getElementById('confidence-tick');
const reasonsList = document.getElementById('reasons-list');
const cleanControls = document.getElementById('clean-controls');
const verificationTable = document.getElementById('verification-table');
const overlayLayer = document.getElementById('overlay-layer');
const cleanButton = document.getElementById('clean-button');
const blurMode = document.getElementById('blur-mode');
const analysisStatus = document.getElementById('analysis-status');

let currentReport = null;
let selectedRegions = new Set();

fileInput.addEventListener('change', async () => {
  const file = fileInput.files[0];
  if (!file) return;
  await analyzeFile(file);
});

uploadBox.addEventListener('dragover', (event) => {
  event.preventDefault();
  uploadBox.classList.add('drag-over');
});

uploadBox.addEventListener('drop', async (event) => {
  event.preventDefault();
  uploadBox.classList.remove('drag-over');
  const file = event.dataTransfer.files[0];
  if (file) await analyzeFile(file);
});

async function analyzeFile(file) {
  if (!file) return;
  analysisStatus.classList.remove('hidden');
  const formData = new FormData();
  formData.append('file', file);

  try {
    const response = await fetch('/analyze', { method: 'POST', body: formData });
    if (!response.ok) {
      const error = await response.json().catch(() => ({ detail: 'Upload failed.' }));
      throw new Error(error.detail || 'Upload failed.');
    }

    const json = await response.json();
    currentReport = json.report;
    renderImagePreview(file, currentReport.regions);
    renderPrivacyTable(currentReport.privacy);
    renderAuthenticity(currentReport.authenticity);
    renderCleanControls();
    reportEmpty.classList.add('hidden');
    privacyTable.classList.remove('hidden');
    authenticityPanel.classList.remove('hidden');
    cleanControls.classList.remove('hidden');
  } catch (error) {
    alert(error.message || 'Analysis could not complete.');
  } finally {
    analysisStatus.classList.add('hidden');
  }
}

function renderImagePreview(file, regions) {
  const reader = new FileReader();
  reader.onload = () => {
    previewImage.src = reader.result;
    imageStage.classList.remove('hidden');
    drawRegionOverlay(regions);
  };
  reader.readAsDataURL(file);
}

function drawRegionOverlay(regions) {
  overlayLayer.innerHTML = '';
  selectedRegions = new Set();
  const allRegions = [...(regions.faces || []), ...(regions.plates || [])];

  for (const region of allRegions) {
    const box = document.createElement('button');
    box.type = 'button';
    box.className = 'region-box';
    box.title = region.label;
    const imageWidth = previewImage.naturalWidth || 1000;
    const imageHeight = previewImage.naturalHeight || 1000;
    const stageWidth = previewImage.clientWidth || imageWidth;
    const stageHeight = previewImage.clientHeight || imageHeight;

    const x = (region.x / imageWidth) * 100;
    const y = (region.y / imageHeight) * 100;
    const w = (region.w / imageWidth) * 100;
    const h = (region.h / imageHeight) * 100;

    box.style.left = `${x}%`;
    box.style.top = `${y}%`;
    box.style.width = `${w}%`;
    box.style.height = `${h}%`;
    box.dataset.regionId = `${region.label}:${region.x}:${region.y}:${region.w}:${region.h}`;
    box.addEventListener('click', () => {
      if (selectedRegions.has(box.dataset.regionId)) {
        selectedRegions.delete(box.dataset.regionId);
        box.classList.remove('selected');
      } else {
        selectedRegions.add(box.dataset.regionId);
        box.classList.add('selected');
      }
    });

    overlayLayer.appendChild(box);
  }
}

function renderPrivacyTable(findings) {
  if (!findings || findings.length === 0) {
    privacyTable.classList.add('hidden');
    reportEmpty.classList.remove('hidden');
    return;
  }

  const header = `
    <table class="report-table"> 
      <thead>
        <tr>
          <th>Finding</th>
          <th>Value</th>
          <th>Severity</th>
        </tr>
      </thead>
      <tbody>
        ${findings.map(item => `
          <tr>
            <td>${escapeHtml(item.label)}</td>
            <td>${escapeHtml(item.value)}</td>
            <td><span class="severity-tag ${severityClass(item.severity)}">${escapeHtml(item.severity.toUpperCase())}</span></td>
          </tr>
        `).join('')}
      </tbody>
    </table>
  `;
  privacyTable.innerHTML = header;
  privacyTable.classList.remove('hidden');
  reportEmpty.classList.add('hidden');
}

function severityClass(severity) {
  return severity.toLowerCase();
}

function renderAuthenticity(auth) {
  if (!auth) return;
  verdictText.textContent = auth.verdict || 'Inconclusive';
  const confidence = Math.max(0, Math.min(100, auth.confidence || 0));
  confidenceTick.style.width = `${confidence}%`;
  confidenceValue.textContent = `${confidence}%`;

  reasonsList.innerHTML = '';
  for (const reason of auth.reasons || []) {
    const li = document.createElement('li');
    li.textContent = reason;
    reasonsList.appendChild(li);
  }

  authenticityPanel.classList.remove('hidden');
}

function renderCleanControls() {
  if (!currentReport) return;
  cleanControls.classList.remove('hidden');
}

cleanButton.addEventListener('click', async () => {
  if (!fileInput.files[0]) {
    alert('Upload a photo before cleaning.');
    return;
  }

  const selected = [...document.querySelectorAll('input[type="checkbox"][value]:checked')].map(item => item.value);
  const formData = new FormData();
  formData.append('file', fileInput.files[0]);
  formData.append('selected_fields', JSON.stringify(selected));
  formData.append('keep_authenticity_fields', document.getElementById('keep-authenticity').checked ? 'true' : 'false');
  formData.append('blur_mode', blurMode.value);

  const regionPayload = [];
  const regionIds = [...selectedRegions];
  const allRegions = [...(currentReport.regions.faces || []), ...(currentReport.regions.plates || [])];
  for (const region of allRegions) {
    const id = `${region.label}:${region.x}:${region.y}:${region.w}:${region.h}`;
    const selectedByUser = regionIds.includes(id);
    if (!selectedByUser) continue;
    regionPayload.push({
      x: region.x,
      y: region.y,
      w: region.w,
      h: region.h,
      kind: region.label,
    });
  }
  formData.append('blur_regions', JSON.stringify(regionPayload));

  try {
    const response = await fetch('/clean', { method: 'POST', body: formData });
    if (!response.ok) {
      const error = await response.json().catch(() => ({ detail: 'Clean failed.' }));
      throw new Error(error.detail || 'Clean failed.');
    }

    const json = await response.json();
    const binary = Uint8Array.from(atob(json.cleaned_image_base64), c => c.charCodeAt(0));
    const blob = new Blob([binary], { type: fileInput.files[0].type || 'image/jpeg' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'truthvision_lens_cleaned.jpg';
    anchor.click();
    URL.revokeObjectURL(url);

    renderVerification(json.verification);
  } catch (error) {
    alert(error.message || 'Cleaning failed.');
  }
});

function renderVerification(rows) {
  if (!rows || rows.length === 0) {
    verificationTable.classList.add('hidden');
    return;
  }

  verificationTable.innerHTML = `
    <table class="verification-table">
      <thead>
        <tr>
          <th>Field</th>
          <th>Before</th>
          <th>After</th>
          <th>Status</th>
        </tr>
      </thead>
      <tbody>
        ${rows.map(item => `
          <tr>
            <td>${escapeHtml(item.field)}</td>
            <td>${escapeHtml(item.before)}</td>
            <td>${escapeHtml(item.after)}</td>
            <td>${escapeHtml(item.status)}</td>
          </tr>
        `).join('')}
      </tbody>
    </table>
  `;
  verificationTable.classList.remove('hidden');
}

function escapeHtml(input) {
  return String(input)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

