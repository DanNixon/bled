import { getBufferRange } from '../services/ble.js';
import { log } from '../services/logger.js';
import { escapeHtml, formatBytes, renderHexDump } from '../utils/formatters.js';

function getBufferFilename(device, target) {
  const sanitize = (str) => (str || '').toLowerCase().replace(/[^a-z0-9_-]/g, '_');
  const devName = sanitize(device?.displayName || 'device');
  if (target?.fixture) {
    const fixName = sanitize(target.fixture.name);
    if (target.span) {
      return `${devName}_${fixName}_ch${target.span.channel}_s${target.span.start}_buffer.bin`;
    }
    return `${devName}_${fixName}_buffer.bin`;
  }
  const chName = sanitize(target?.channel?.name || `ch${target?.channel?.index ?? 0}`);
  return `${devName}_${chName}_buffer.bin`;
}

/** LED buffer save/load controls for a specific fixture, span, or channel. */
export class BledChannelBuffer extends HTMLElement {
  constructor() {
    super();
    this.target = null;
    this.readBufferCache = null;
    this.stagedFileData = null;
    this.stagedFileName = '';
    this._currentTargetRange = null;
  }

  connectedCallback() {
    this.innerHTML = `
      <div class="card">
        <div class="card-header">
          <div>
            <span class="card-title">Buffer Data</span>
            <span id="targetSubtitle" class="card-subtitle" style="display: block;"></span>
          </div>
        </div>

        <div class="info-list">
          <div class="info-box">
            <div class="info-box-label">Buffer Range</div>
            <div id="bufRange" class="info-box-value">—</div>
          </div>
          <div class="info-box">
            <div class="info-box-label">Target Size</div>
            <div id="bufSize" class="info-box-value">—</div>
          </div>
          <div class="info-box">
            <div class="info-box-label">Pixel Count</div>
            <div id="bufPixels" class="info-box-value">—</div>
          </div>
        </div>

        <!-- Transfer progress -->
        <div id="channelBufProgressWrap" class="progress-bar-wrap">
          <div id="channelBufProgressBar" class="progress-bar-fill"></div>
        </div>

        <!-- Save Buffer Data -->
        <div style="border-top: 1px solid var(--card-border); padding-top: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem;">
          <label>Save Buffer Data</label>
          <span class="card-subtitle">Read current LED buffer data from device and save as binary file</span>
          <div class="form-row">
            <button id="btnReadBuffer" class="btn btn-secondary btn-sm">Read & Preview</button>
            <button id="btnSaveBuffer" class="btn btn-secondary btn-sm">Save to Disk (.bin)</button>
          </div>
          <pre id="readHexPreview" style="display: none; background: var(--code-bg); border: 1px solid var(--card-border); border-radius: 4px; padding: 0.5rem; font-size: 0.75rem; max-height: 120px; overflow: auto; color: var(--text-muted); font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;"></pre>
        </div>

        <!-- Load Buffer Data -->
        <div style="border-top: 1px solid var(--card-border); padding-top: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem;">
          <label>Load Buffer Data</label>
          <span class="card-subtitle">Upload binary LED buffer data (.bin) to this target</span>
          <div class="form-row">
            <div class="form-group" style="flex: 1;">
              <input type="file" id="bufFileInput" accept=".bin,application/octet-stream" style="padding: 0.35rem 0.5rem; font-size: 0.75rem;">
            </div>
          </div>
          <div id="fileNotice" style="display: none; font-size: 0.8125rem; color: var(--text-muted);"></div>
          <pre id="loadHexPreview" style="display: none; background: var(--code-bg); border: 1px solid var(--card-border); border-radius: 4px; padding: 0.5rem; font-size: 0.75rem; max-height: 120px; overflow: auto; color: var(--text-muted); font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;"></pre>
          <div class="form-row" style="justify-content: space-between; align-items: center; margin-top: 0.25rem;">
            <label style="display: flex; align-items: center; gap: 0.375rem; cursor: pointer;">
              <input type="checkbox" id="autoCommitCheck" checked> Auto-commit on write
            </label>
            <button id="btnWriteBuffer" class="btn btn-primary btn-sm" disabled>Upload to LED Buffer</button>
          </div>
        </div>
      </div>
    `;

    this.targetSubtitle = this.querySelector('#targetSubtitle');
    this.bufRange = this.querySelector('#bufRange');
    this.bufSize = this.querySelector('#bufSize');
    this.bufPixels = this.querySelector('#bufPixels');
    this.bufProgressWrap = this.querySelector('#channelBufProgressWrap');
    this.bufProgressBar = this.querySelector('#channelBufProgressBar');
    this.btnReadBuffer = this.querySelector('#btnReadBuffer');
    this.btnSaveBuffer = this.querySelector('#btnSaveBuffer');
    this.readHexPreview = this.querySelector('#readHexPreview');
    this.bufFileInput = this.querySelector('#bufFileInput');
    this.fileNotice = this.querySelector('#fileNotice');
    this.loadHexPreview = this.querySelector('#loadHexPreview');
    this.autoCommitCheck = this.querySelector('#autoCommitCheck');
    this.btnWriteBuffer = this.querySelector('#btnWriteBuffer');

    this.btnReadBuffer.onclick = () => this.readTargetBuffer();
    this.btnSaveBuffer.onclick = () => this.saveTargetBuffer();
    this.bufFileInput.onchange = (e) => this.onFileSelected(e.target.files[0]);
    this.btnWriteBuffer.onclick = () => this.writeTargetBuffer();

    if (this._pendingTarget) {
      this.setTarget(this._pendingTarget);
      this._pendingTarget = null;
    }
  }

  /**
   * Set target fixture, span, or channel to save/load.
   */
  setTarget(target) {
    if (!this.targetSubtitle) {
      this._pendingTarget = target;
      return;
    }
    this.target = target;
    this.readBufferCache = null;
    this.stagedFileData = null;
    this.stagedFileName = '';

    // Reset UI state for new target
    this.readHexPreview.style.display = 'none';
    this.readHexPreview.textContent = '';
    this.loadHexPreview.style.display = 'none';
    this.loadHexPreview.textContent = '';
    this.fileNotice.style.display = 'none';
    this.fileNotice.textContent = '';
    this.bufFileInput.value = '';
    this.btnWriteBuffer.disabled = true;

    const { fixture, span, channel, segment } = target;
    let range = { startPixel: 0, pixelCount: 0, byteOffset: 0, byteLength: 0 };

    if (fixture) {
      if (span) {
        range = {
          startPixel: span.start,
          pixelCount: span.length,
          byteOffset: span.byteOffset,
          byteLength: span.byteLength,
        };
        this.targetSubtitle.textContent =
          `${fixture.name} / Span (Ch ${span.channel}): pixels ${span.start}–${span.start + span.length - 1} (buffer: ${range.byteOffset}–${range.byteOffset + range.byteLength - 1})`;
      } else {
        range = {
          startPixel: 0,
          pixelCount: fixture.totalPixels,
          byteOffset: fixture.byteOffset,
          byteLength: fixture.totalBytes,
        };
        this.targetSubtitle.textContent =
          `${fixture.name}: entire fixture (${fixture.totalPixels} px, ${fixture.spans.length} span${fixture.spans.length === 1 ? '' : 's'})`;
      }
    } else if (channel) {
      range = getBufferRange(this.device?.config, { channel, segment });
      if (segment) {
        this.targetSubtitle.textContent =
          `${channel.name} / ${segment.name}: pixels ${segment.start}–${segment.start + segment.length - 1}`;
      } else {
        this.targetSubtitle.textContent =
          `${channel.name}: entire channel (${channel.length} pixels)`;
      }
    }

    this._currentTargetRange = range;
    const { byteOffset, byteLength, pixelCount } = range;

    if (pixelCount === 0 || byteLength === 0) {
      this.bufRange.textContent = '—';
      this.bufSize.textContent = '0 B';
      this.bufPixels.textContent = '0 px';
      this.btnReadBuffer.disabled = true;
      this.btnSaveBuffer.disabled = true;
      this.bufFileInput.disabled = true;
      this.fileNotice.textContent = 'Target has 0 configured pixels.';
      this.fileNotice.style.display = 'block';
    } else {
      this.bufRange.textContent = `${byteOffset} – ${byteOffset + byteLength - 1}`;
      this.bufSize.textContent = `${formatBytes(byteLength)} (${byteLength} B)`;
      this.bufPixels.textContent = `${pixelCount} px`;
      this.btnReadBuffer.disabled = false;
      this.btnSaveBuffer.disabled = false;
      this.bufFileInput.disabled = false;
    }
  }

  async readTargetBuffer() {
    if (!this._currentTargetRange || this._currentTargetRange.byteLength === 0) return;
    const { byteOffset, byteLength, pixelCount } = this._currentTargetRange;

    try {
      this.bufProgressWrap.style.display = 'block';
      this.bufProgressBar.style.width = '0%';

      const combined = await this.device.readBufferRange(byteOffset, byteLength, (pct) => {
        this.bufProgressBar.style.width = `${pct}%`;
      });

      this.readBufferCache = combined;
      this.readHexPreview.textContent = renderHexDump(combined, 512);
      this.readHexPreview.style.display = 'block';

      const targetDesc = this.target?.fixture?.name || this.target?.channel?.name || 'target';
      log(`Read ${combined.length} bytes (${pixelCount} pixels) from ${targetDesc} at offset ${byteOffset}.`, 'success');

      setTimeout(() => { this.bufProgressWrap.style.display = 'none'; }, 400);
      return combined;
    } catch (err) {
      this.bufProgressWrap.style.display = 'none';
      log(`Failed reading target buffer: ${err.message}`, 'error');
    }
  }

  async saveTargetBuffer() {
    if (!this.readBufferCache) {
      await this.readTargetBuffer();
    }
    if (!this.readBufferCache) return;
    const blob = new Blob([this.readBufferCache], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    const filename = getBufferFilename(this.device, this.target);
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    log(`Saved ${formatBytes(this.readBufferCache.length)} as "${filename}".`, 'success');
  }

  onFileSelected(file) {
    if (!file) {
      this.stagedFileData = null;
      this.stagedFileName = '';
      this.loadHexPreview.style.display = 'none';
      this.fileNotice.style.display = 'none';
      this.btnWriteBuffer.disabled = true;
      return;
    }

    const { byteLength, pixelCount } = this._currentTargetRange;
    const reader = new FileReader();
    reader.onload = () => {
      const data = new Uint8Array(reader.result);
      this.stagedFileData = data;
      this.stagedFileName = file.name;
      this.loadHexPreview.textContent = renderHexDump(data, 512);
      this.loadHexPreview.style.display = 'block';

      this.fileNotice.style.display = 'block';
      if (data.length === byteLength) {
        this.fileNotice.innerHTML =
          `Selected <strong>${escapeHtml(file.name)}</strong>: ${formatBytes(data.length)} (<span style="color: var(--success);">exact match</span> for ${pixelCount} pixels).`;
      } else if (data.length > byteLength) {
        this.fileNotice.innerHTML =
          `Selected <strong>${escapeHtml(file.name)}</strong>: ${formatBytes(data.length)} (<span style="color: var(--warning);">larger than target ${formatBytes(byteLength)}</span>; will truncate to ${pixelCount} pixels / ${byteLength} B).`;
      } else {
        const filePixels = Math.floor(data.length / 3);
        this.fileNotice.innerHTML =
          `Selected <strong>${escapeHtml(file.name)}</strong>: ${formatBytes(data.length)} (<span style="color: var(--warning);">partial data</span>: ${filePixels} of ${pixelCount} pixels).`;
      }

      this.btnWriteBuffer.disabled = data.length === 0 || byteLength === 0;
    };
    reader.readAsArrayBuffer(file);
  }

  async writeTargetBuffer() {
    if (!this.stagedFileData || !this._currentTargetRange) return;
    const { byteOffset, byteLength } = this._currentTargetRange;
    if (byteLength === 0) return;

    // Truncate to target byteLength if file is larger
    const toUpload = this.stagedFileData.length > byteLength
      ? this.stagedFileData.subarray(0, byteLength)
      : this.stagedFileData;

    try {
      this.bufProgressWrap.style.display = 'block';
      this.bufProgressBar.style.width = '5%';

      await this.device.writeBuffer(byteOffset, toUpload, (pct) => {
        this.bufProgressBar.style.width = `${pct}%`;
      });

      const targetDesc = this.target?.fixture?.name || this.target?.channel?.name || 'target';
      log(`Uploaded ${toUpload.length} bytes to ${targetDesc} at buffer offset ${byteOffset}.`, 'success');

      if (this.autoCommitCheck.checked) {
        await this.device.commit();
        log(`Rendered/committed LED buffer.`, 'success');
      }

      setTimeout(() => { this.bufProgressWrap.style.display = 'none'; }, 400);
    } catch (err) {
      this.bufProgressWrap.style.display = 'none';
      log(`Failed writing to target buffer: ${err.message}`, 'error');
    }
  }
}

customElements.define('bled-channel-buffer', BledChannelBuffer);
