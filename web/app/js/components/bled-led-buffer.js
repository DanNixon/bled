import { log } from '../services/logger.js';
import { formatBytes, renderHexDump } from '../utils/formatters.js';

/** LED buffer controls for a device. Set `.device` before attaching. */
export class BledLedBuffer extends HTMLElement {
  constructor() {
    super();
    this.readBufferCache = null;
  }

  connectedCallback() {
    this.innerHTML = `
      <div class="card">
        <div class="card-header">
          <div>
            <span class="card-title">LED Data Buffer</span>
            <span class="card-subtitle" style="display: block;">Low-level frame buffer memory access</span>
          </div>
          <button id="btnReadBufferInfo" class="btn btn-secondary btn-sm">Check Info</button>
        </div>

        <div class="info-list">
          <div class="info-box">
            <div class="info-box-label">Buffer Capacity</div>
            <div id="bufCapacity" class="info-box-value">—</div>
          </div>
          <div class="info-box">
            <div class="info-box-label">Current Buffer Size</div>
            <div id="bufSize" class="info-box-value">—</div>
          </div>
        </div>

        <!-- Transfer progress -->
        <div id="bufProgressWrap" class="progress-bar-wrap">
          <div id="bufProgressBar" class="progress-bar-fill"></div>
        </div>

        <!-- Buffer Read -->
        <div style="border-top: 1px solid var(--card-border); padding-top: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem;">
          <label>Read Buffer</label>
          <div class="form-row">
            <button id="btnReadBuffer" class="btn btn-secondary btn-sm">Read Entire Buffer</button>
            <button id="btnDownloadBuffer" class="btn btn-secondary btn-sm" style="display: none;">Download .bin</button>
          </div>
          <pre id="bufferHexPreview" style="display: none; background: var(--code-bg); border: 1px solid var(--card-border); border-radius: 4px; padding: 0.5rem; font-size: 0.75rem; max-height: 100px; overflow: auto; color: var(--text-muted);"></pre>
        </div>

        <!-- Buffer Write -->
        <div style="border-top: 1px solid var(--card-border); padding-top: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem;">
          <label>Write Buffer</label>
          <div class="form-row">
            <div class="form-group" style="width: 110px;">
              <label for="bufWriteOffset">Offset (bytes)</label>
              <input type="number" id="bufWriteOffset" value="0" min="0">
            </div>
            <div class="form-group" style="flex: 1;">
              <label>Select .bin File</label>
              <input type="file" id="bufFileInput" style="padding: 0.35rem 0.5rem; font-size: 0.75rem;">
            </div>
          </div>
          <button id="btnWriteBuffer" class="btn btn-primary btn-sm" style="margin-top: 0.25rem;">Upload to LED Buffer</button>
        </div>
      </div>
    `;

    this.btnReadBufferInfo = this.querySelector('#btnReadBufferInfo');
    this.bufCapacity = this.querySelector('#bufCapacity');
    this.bufSize = this.querySelector('#bufSize');
    this.bufProgressWrap = this.querySelector('#bufProgressWrap');
    this.bufProgressBar = this.querySelector('#bufProgressBar');
    this.btnReadBuffer = this.querySelector('#btnReadBuffer');
    this.btnDownloadBuffer = this.querySelector('#btnDownloadBuffer');
    this.bufferHexPreview = this.querySelector('#bufferHexPreview');
    this.bufWriteOffset = this.querySelector('#bufWriteOffset');
    this.bufFileInput = this.querySelector('#bufFileInput');
    this.btnWriteBuffer = this.querySelector('#btnWriteBuffer');

    this.btnReadBufferInfo.onclick = () => this.readInfo();

    this.btnReadBuffer.onclick = async () => {
      try {
        const info = await this.readInfo();
        if (!info || info.size === 0) {
          log("LED buffer is empty or size could not be determined.", 'error');
          return;
        }

        this.bufProgressWrap.style.display = 'block';
        this.bufProgressBar.style.width = '0%';

        const combined = await this.device.readBuffer(info.size, (pct) => {
          this.bufProgressBar.style.width = `${pct}%`;
        });

        this.readBufferCache = combined;
        this.bufferHexPreview.textContent = renderHexDump(combined, 512);
        this.bufferHexPreview.style.display = 'block';
        this.btnDownloadBuffer.style.display = 'inline-flex';

        setTimeout(() => { this.bufProgressWrap.style.display = 'none'; }, 400);
      } catch (err) {
        this.bufProgressWrap.style.display = 'none';
        log(`Failed reading LED buffer: ${err.message}`, 'error');
      }
    };

    this.btnDownloadBuffer.onclick = () => {
      if (!this.readBufferCache) return;
      const blob = new Blob([this.readBufferCache], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'led_buffer_dump.bin';
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      log(`Downloaded ${formatBytes(this.readBufferCache.length)} as "led_buffer_dump.bin".`, 'success');
    };

    this.btnWriteBuffer.onclick = () => {
      const file = this.bufFileInput.files[0];
      if (!file) {
        log("Please select a binary file (.bin) to upload to the buffer.", 'error');
        return;
      }
      const offset = parseInt(this.bufWriteOffset.value, 10) || 0;

      const reader = new FileReader();
      reader.onload = async () => {
        try {
          const rawData = new Uint8Array(reader.result);
          this.bufProgressWrap.style.display = 'block';
          this.bufProgressBar.style.width = '5%';

          await this.device.writeBuffer(offset, rawData, (pct) => {
            this.bufProgressBar.style.width = `${pct}%`;
          });

          setTimeout(() => { this.bufProgressWrap.style.display = 'none'; }, 400);
          await this.readInfo();
        } catch (err) {
          this.bufProgressWrap.style.display = 'none';
          log(`Failed writing to LED buffer: ${err.message}`, 'error');
        }
      };
      reader.readAsArrayBuffer(file);
    };

    this.readInfo();
  }

  async readInfo() {
    try {
      const info = await this.device.readBufferInfo();
      this.bufCapacity.textContent = formatBytes(info.capacity);
      this.bufSize.textContent = formatBytes(info.size);
      return info;
    } catch (err) {
      log(`Failed reading LED Buffer Info: ${err.message}`, 'error');
    }
  }
}

customElements.define('bled-led-buffer', BledLedBuffer);
