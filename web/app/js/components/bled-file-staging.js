import { log } from '../services/logger.js';
import { formatBytes, renderHexDump } from '../utils/formatters.js';

/** SD card file operations for a device. Set `.device` before attaching. */
export class BledFileStaging extends HTMLElement {
  constructor() {
    super();
    this.stagedData = new Uint8Array(0);
    this.stagedFilename = '';
    this.isHexView = false;
  }

  connectedCallback() {
    this.innerHTML = `
      <div class="card full-width">
        <div class="card-header">
          <div>
            <span class="card-title">File Management</span>
            <span class="card-subtitle" style="display:block;">
              Buffer between files on disk and files on device
            </span>
          </div>
          <div class="form-row">
            <input type="file" id="diskFileInput" style="display: none;">
            <button id="btnLoadDisk" class="btn btn-secondary btn-sm">Load from Disk</button>
            <button id="btnSaveDisk" class="btn btn-secondary btn-sm" disabled>Save to Disk</button>
            <button id="btnClearEditor" class="btn btn-secondary btn-sm">Clear Buffer</button>
          </div>
        </div>

        <!-- Remote Path & Device actions toolbar -->
        <div class="form-row" style="justify-content: space-between; align-items: flex-end;">
          <div class="form-group" style="flex: 1; min-width: 250px;">
            <label for="remotePathInput">Remote Path on Device</label>
            <input type="text" id="remotePathInput" placeholder="/config.json" value="/config.json" maxlength="64">
          </div>
          <div class="form-row">
            <button id="btnStatFile" class="btn btn-secondary btn-sm">Stat</button>
            <button id="btnReadFile" class="btn btn-primary btn-sm">Read from Device</button>
            <button id="btnWriteFile" class="btn btn-primary btn-sm">Write to Device</button>
            <button id="btnDeleteFile" class="btn btn-danger btn-sm">Delete on Device</button>
          </div>
        </div>

        <!-- Transfer progress bar -->
        <div id="fileProgressWrap" class="progress-bar-wrap">
          <div id="fileProgressBar" class="progress-bar-fill"></div>
        </div>

        <!-- Editor staging status -->
        <div class="editor-container">
          <div class="editor-meta">
            <div class="editor-meta-tags">
              <span>Staged: <strong id="editorFileName">&lt;Empty&gt;</strong></span>
              <span>Size: <strong id="editorFileSize">0 B</strong></span>
              <span>Mode: <strong id="editorMode">Text (UTF-8)</strong></span>
            </div>
            <div>
              <button id="btnToggleHex" class="btn btn-secondary btn-sm" style="padding: 0.15rem 0.5rem; font-size: 0.75rem;">View Hex</button>
            </div>
          </div>
          <textarea id="textEditor" class="code-editor" placeholder="File contents will appear here. You can load a file from disk, read from device, or type directly..."></textarea>
          <pre id="hexViewer" class="hex-editor" style="display: none;"></pre>
        </div>
      </div>
    `;

    // DOM references
    this.diskFileInput = this.querySelector('#diskFileInput');
    this.btnLoadDisk = this.querySelector('#btnLoadDisk');
    this.btnSaveDisk = this.querySelector('#btnSaveDisk');
    this.btnClearEditor = this.querySelector('#btnClearEditor');
    this.remotePathInput = this.querySelector('#remotePathInput');
    this.btnStatFile = this.querySelector('#btnStatFile');
    this.btnReadFile = this.querySelector('#btnReadFile');
    this.btnWriteFile = this.querySelector('#btnWriteFile');
    this.btnDeleteFile = this.querySelector('#btnDeleteFile');
    this.fileProgressWrap = this.querySelector('#fileProgressWrap');
    this.fileProgressBar = this.querySelector('#fileProgressBar');
    this.editorFileName = this.querySelector('#editorFileName');
    this.editorFileSize = this.querySelector('#editorFileSize');
    this.editorMode = this.querySelector('#editorMode');
    this.btnToggleHex = this.querySelector('#btnToggleHex');
    this.textEditor = this.querySelector('#textEditor');
    this.hexViewer = this.querySelector('#hexViewer');

    // Attach listeners
    this.textEditor.addEventListener('input', () => {
      this.stagedData = new TextEncoder().encode(this.textEditor.value);
      this.editorFileSize.textContent = formatBytes(this.stagedData.length);
      this.btnSaveDisk.disabled = (this.stagedData.length === 0);
    });

    this.btnToggleHex.onclick = () => {
      this.isHexView = !this.isHexView;
      this.updateUI();
    };

    this.btnClearEditor.onclick = () => {
      this.stagedData = new Uint8Array(0);
      this.stagedFilename = '';
      this.textEditor.value = '';
      this.hexViewer.textContent = '';
      this.isHexView = false;
      this.updateUI();
      log("Staging buffer cleared.");
    };

    this.btnLoadDisk.onclick = () => this.diskFileInput.click();
    this.diskFileInput.onchange = (e) => {
      const file = e.target.files[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = () => {
        this.stagedData = new Uint8Array(reader.result);
        this.stagedFilename = file.name;
        if (!this.remotePathInput.value || this.remotePathInput.value === '/config.json') {
          this.remotePathInput.value = `/${file.name}`;
        }
        this.isHexView = false;
        this.updateUI();
        log(`Loaded "${file.name}" (${formatBytes(this.stagedData.length)}) from disk into staging buffer.`, 'success');
      };
      reader.readAsArrayBuffer(file);
      this.diskFileInput.value = '';
    };

    this.btnSaveDisk.onclick = () => {
      if (this.stagedData.length === 0) return;
      const blob = new Blob([this.stagedData], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      const defaultName = this.stagedFilename || this.remotePathInput.value.replace(/^\//, '') || 'staged_file.bin';
      a.download = defaultName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      log(`Saved staging buffer (${formatBytes(this.stagedData.length)}) as "${defaultName}" to disk.`, 'success');
    };

    this.btnStatFile.onclick = async () => {
      const path = this.remotePathInput.value.trim();
      if (!path) {
        log("Please specify a remote path to stat.", 'error');
        return;
      }
      try {
        const stat = await this.device.statFile(path);
        log(`Stat for "${path}": size = ${stat.size} bytes (${formatBytes(stat.size)})`, 'success');
      } catch (err) {
        log(`File stat failed: ${err.message}`, 'error');
      }
    };

    this.btnReadFile.onclick = async () => {
      const path = this.remotePathInput.value.trim();
      if (!path) {
        log("Please specify a remote path to read.", 'error');
        return;
      }
      try {
        this.fileProgressWrap.style.display = 'block';
        this.fileProgressBar.style.width = '0%';

        const data = await this.device.readFile(path, (pct) => {
          this.fileProgressBar.style.width = `${pct}%`;
        });

        this.stagedData = data;
        this.stagedFilename = path.split('/').pop() || path;
        this.isHexView = false;
        this.fileProgressBar.style.width = '100%';
        setTimeout(() => { this.fileProgressWrap.style.display = 'none'; }, 400);

        this.updateUI();
        log(`Successfully read "${path}" (${formatBytes(data.length)}) into staging buffer.`, 'success');
      } catch (err) {
        this.fileProgressWrap.style.display = 'none';
        log(`File read failed: ${err.message}`, 'error');
      }
    };

    this.btnWriteFile.onclick = async () => {
      const path = this.remotePathInput.value.trim();
      if (!path) {
        log("Please enter a target remote path on the device.", 'error');
        return;
      }
      if (!this.isHexView && this.textEditor.style.display !== 'none') {
        this.stagedData = new TextEncoder().encode(this.textEditor.value);
      }
      if (this.stagedData.length === 0) {
        log("Staging buffer is empty. Nothing to write.", 'error');
        return;
      }

      try {
        this.fileProgressWrap.style.display = 'block';
        this.fileProgressBar.style.width = '5%';

        await this.device.writeFile(path, this.stagedData, (pct) => {
          this.fileProgressBar.style.width = `${pct}%`;
        });

        setTimeout(() => { this.fileProgressWrap.style.display = 'none'; }, 400);
      } catch (err) {
        this.fileProgressWrap.style.display = 'none';
        log(`File write failed: ${err.message}`, 'error');
      }
    };

    this.btnDeleteFile.onclick = async () => {
      const path = this.remotePathInput.value.trim();
      if (!path) {
        log("Please enter a remote path to delete.", 'error');
        return;
      }
      if (!confirm(`Are you sure you want to delete "${path}" on the connected device?`)) return;

      try {
        await this.device.deleteFile(path);
      } catch (err) {
        log(`File deletion failed: ${err.message}`, 'error');
      }
    };

    this.updateUI();
  }

  updateUI() {
    this.editorFileName.textContent = this.stagedFilename || (this.stagedData.length > 0 ? '<In-Memory Buffer>' : '<Empty>');
    this.editorFileSize.textContent = formatBytes(this.stagedData.length);
    this.btnSaveDisk.disabled = (this.stagedData.length === 0);

    let isUtf8 = true;
    let textContent = '';
    try {
      textContent = new TextDecoder('utf-8', { fatal: true }).decode(this.stagedData);
    } catch (_) {
      isUtf8 = false;
    }

    if (isUtf8) {
      this.editorMode.textContent = 'Text (UTF-8)';
      if (!this.isHexView) {
        this.textEditor.value = textContent;
        this.textEditor.style.display = 'block';
        this.hexViewer.style.display = 'none';
        this.btnToggleHex.textContent = 'View Hex';
      } else {
        this.hexViewer.textContent = renderHexDump(this.stagedData);
        this.textEditor.style.display = 'none';
        this.hexViewer.style.display = 'block';
        this.btnToggleHex.textContent = 'View Text';
      }
    } else {
      this.editorMode.textContent = 'Binary';
      this.hexViewer.textContent = renderHexDump(this.stagedData);
      this.textEditor.style.display = 'none';
      this.hexViewer.style.display = 'block';
      this.btnToggleHex.textContent = 'Binary (Hex Only)';
    }
  }
}

customElements.define('bled-file-staging', BledFileStaging);
