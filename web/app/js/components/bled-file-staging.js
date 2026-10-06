import { log } from '../services/logger.js';
import { escapeHtml, formatBytes, renderHexDump } from '../utils/formatters.js';

function joinPath(dir, name) {
  const d = dir.replace(/\/+$/, '');
  return `${d}/${name.replace(/^\/+/, '')}`;
}

function parentDir(path) {
  const normalized = path.replace(/\/+$/, '');
  const lastSlash = normalized.lastIndexOf('/');
  if (lastSlash <= 0) return '/';
  return normalized.slice(0, lastSlash);
}

/** SD card file operations for a device. Set `.device` before attaching. */
export class BledFileStaging extends HTMLElement {
  constructor() {
    super();
    this._device = null;
    this.stagedData = new Uint8Array(0);
    this.stagedFilename = '';
    this.isHexView = false;
    this.dirEntries = [];
  }

  get device() {
    return this._device;
  }

  set device(dev) {
    this._device = dev;
    if (this.isConnected && dev) {
      this.loadDirectory();
    }
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

        <!-- Device Directory Browser -->
        <div class="dir-section">
          <div class="form-row" style="justify-content: space-between; align-items: center;">
            <div class="form-row" style="align-items: center; flex: 1;">
              <label for="dirPathInput" style="white-space: nowrap;">Directory:</label>
              <input type="text" id="dirPathInput" value="/" placeholder="/" maxlength="64" style="max-width: 220px; padding: 0.25rem 0.5rem;">
              <button id="btnListDir" class="btn btn-secondary btn-sm">Refresh List</button>
              <button id="btnDirUp" class="btn btn-secondary btn-sm" title="Go up one directory">⬆ Up</button>
            </div>
            <span id="dirSummary" class="card-subtitle"></span>
          </div>

          <div id="dirTableWrap" class="dir-table-wrap">
            <table class="dir-table">
              <thead>
                <tr>
                  <th style="width: 28px;"></th>
                  <th>Name</th>
                  <th style="width: 100px;">Size</th>
                  <th style="width: 140px; text-align: right;">Actions</th>
                </tr>
              </thead>
              <tbody id="dirTableBody">
                <tr><td colspan="4" class="dir-empty">Connect a device to view files.</td></tr>
              </tbody>
            </table>
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
    this.dirPathInput = this.querySelector('#dirPathInput');
    this.btnListDir = this.querySelector('#btnListDir');
    this.btnDirUp = this.querySelector('#btnDirUp');
    this.dirSummary = this.querySelector('#dirSummary');
    this.dirTableBody = this.querySelector('#dirTableBody');
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

    this.btnListDir.onclick = () => this.loadDirectory();
    this.btnDirUp.onclick = () => {
      const cur = this.dirPathInput.value.trim() || '/';
      this.dirPathInput.value = parentDir(cur);
      this.loadDirectory();
    };
    this.dirPathInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        this.loadDirectory();
      }
    });

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
        this.loadDirectory();
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
        this.loadDirectory();
      } catch (err) {
        log(`File deletion failed: ${err.message}`, 'error');
      }
    };

    this.updateUI();
    if (this.device) {
      this.loadDirectory();
    }
  }

  async loadDirectory() {
    if (!this.device) return;
    const path = this.dirPathInput.value.trim() || '/';
    this.dirTableBody.innerHTML = '<tr><td colspan="4" class="dir-empty">Listing directory…</td></tr>';
    this.btnListDir.disabled = true;

    try {
      const entries = await this.device.listDirectory(path);
      entries.sort((a, b) => {
        const aIsDir = a.kind === 'Directory';
        const bIsDir = b.kind === 'Directory';
        if (aIsDir !== bIsDir) return aIsDir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      this.dirEntries = entries;
      this.renderDirectoryEntries(path, entries);
    } catch (err) {
      log(`Failed to list directory "${path}": ${err.message}`, 'error');
      this.dirTableBody.innerHTML = `<tr><td colspan="4" class="dir-empty" style="color: var(--danger);">Error: ${escapeHtml(err.message)}</td></tr>`;
      this.dirSummary.textContent = 'Error';
    } finally {
      this.btnListDir.disabled = false;
    }
  }

  renderDirectoryEntries(currentDir, entries) {
    this.dirSummary.textContent = `${entries.length} ${entries.length === 1 ? 'item' : 'items'}`;
    if (entries.length === 0) {
      this.dirTableBody.innerHTML = '<tr><td colspan="4" class="dir-empty">Directory is empty.</td></tr>';
      return;
    }

    this.dirTableBody.innerHTML = entries.map((entry) => {
      const fullPath = joinPath(currentDir, entry.name);
      const isDir = entry.kind === 'Directory';
      const size = (!isDir && entry.kind?.File?.size != null) ? Number(entry.kind.File.size) : 0;
      const icon = isDir ? '📁' : '📄';
      const sizeStr = isDir ? '—' : formatBytes(size);
      const actions = isDir
        ? `<button class="btn btn-secondary btn-sm btn-open-dir" data-path="${escapeHtml(fullPath)}">Open</button>`
        : `<button class="btn btn-secondary btn-sm btn-select-file" data-path="${escapeHtml(fullPath)}">Select</button>
           <button class="btn btn-primary btn-sm btn-read-file" data-path="${escapeHtml(fullPath)}">Read</button>`;

      return `
        <tr>
          <td style="text-align: center;">${icon}</td>
          <td><span class="dir-row-name" data-path="${escapeHtml(fullPath)}" data-is-dir="${isDir}">${escapeHtml(entry.name)}</span></td>
          <td style="color: var(--text-muted);">${sizeStr}</td>
          <td style="text-align: right;">
            <div class="form-row" style="justify-content: flex-end; gap: 0.25rem;">
              ${actions}
            </div>
          </td>
        </tr>
      `;
    }).join('');

    this.dirTableBody.querySelectorAll('.dir-row-name').forEach((el) => {
      el.onclick = () => {
        const fullPath = el.dataset.path;
        const isDir = el.dataset.isDir === 'true';
        if (isDir) {
          this.dirPathInput.value = fullPath;
          this.loadDirectory();
        } else {
          this.remotePathInput.value = fullPath;
        }
      };
    });

    this.dirTableBody.querySelectorAll('.btn-open-dir').forEach((el) => {
      el.onclick = () => {
        this.dirPathInput.value = el.dataset.path;
        this.loadDirectory();
      };
    });

    this.dirTableBody.querySelectorAll('.btn-select-file').forEach((el) => {
      el.onclick = () => {
        this.remotePathInput.value = el.dataset.path;
      };
    });

    this.dirTableBody.querySelectorAll('.btn-read-file').forEach((el) => {
      el.onclick = () => {
        this.remotePathInput.value = el.dataset.path;
        this.btnReadFile.click();
      };
    });
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
