const fileInput = document.getElementById('file-input');
const uploadBox = document.getElementById('upload-box');
const imageStage = document.getElementById('image-stage');
const previewImage = document.getElementById('preview-image');
const reportEmpty = document.getElementById('report-empty');
const reportTable = document.getElementById('report-table');
const authPanel = document.getElementById('auth-panel');
const verdictText = document.getElementById('verdict-text');
const confidenceTick = document.getElementById('confidence-tick');
const confidenceText = document.getElementById('confidence-text');
const reasonsList = document.getElementById('reasons');
const cleanBox = document.getElementById('clean-box');
const verificationTable = document.getElementById('verification-table');
const cleanButton = document.getElementById('clean-button');

fileInput.addEventListener('change', async (event) => {
  const file = event.target.files?.[0];
  if (!file) return;

  const form = new FormData();
  form.append('file', file);

  const response = await fetch('/analyze', { method: 'POST', body: form });
  const data = await response.json();
  const report = data.report || data;

  previewImage.src = URL.createObjectURL(file);
  imageStage.classList.remove('hidden');
  reportEmpty.classList.add('hidden');
  cleanBox.classList.remove('hidden');

  const findings = report.privacy || [];
  reportTable.classList.remove('hidden');
  reportTable.innerHTML = `<thead><tr><th>Field</th><th>Value</th><th>Severity</th></tr></thead><tbody>${findings.map((f) => `<tr><td>${f.label}</td><td>${f.value}</td><td>${f.severity}</td></tr>`).join('')}</tbody>`;

  const authenticity = report.authenticity || {};
  verdictText.textContent = authenticity.verdict || 'Inconclusive';
  confidenceText.textContent = `${authenticity.confidence || 0}% confidence`;
  confidenceTick.style.width = `${authenticity.confidence || 0}%`;
  reasonsList.innerHTML = (authenticity.reasons || []).map((r) => `<li>${r}</li>`).join('');
  authPanel.classList.remove('hidden');
});

cleanButton.addEventListener('click', async () => {
  const file = fileInput.files?.[0];
  if (!file) return;

  const selected = [...document.querySelectorAll('input[type="checkbox"]:checked')].map((el) => el.value);
  const form = new FormData();
  form.append('file', file);
  form.append('selected_fields', JSON.stringify(selected));
  form.append('keep_authenticity_fields', 'false');
  form.append('blur_regions', JSON.stringify([]));
  form.append('blur_mode', 'gaussian');

  const response = await fetch('/clean', { method: 'POST', body: form });
  const data = await response.json();

  verificationTable.classList.remove('hidden');
  verificationTable.innerHTML = `<thead><tr><th>Field</th><th>Before</th><th>After</th><th>Status</th></tr></thead><tbody>${(data.verification || []).map((r) => `<tr><td>${r.field}</td><td>${r.before}</td><td>${r.after}</td><td>${r.status}</td></tr>`).join('')}</tbody>`;

  if (data.cleaned_image_base64) {
    const url = `data:image/jpeg;base64,${data.cleaned_image_base64}`;
    previewImage.src = url;
  }
});
