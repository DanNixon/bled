import { escapeHtml } from '../utils/formatters.js';
import { log } from '../services/logger.js';

function joinPath(dir, name) {
  return `${dir.replace(/\/+$/, '')}/${name.replace(/^\/+/, '')}` || `/${name}`;
}

function formatDuration(durationMs) {
  if (durationMs == null) return 'Indefinite';
  const ms = Number(durationMs);
  if (!Number.isFinite(ms)) return String(durationMs);
  if (ms >= 1000 && ms % 1000 === 0) {
    return `${ms / 1000}s (${ms.toLocaleString()} ms)`;
  }
  return `${ms.toLocaleString()} ms`;
}

function formatAfterAction(after) {
  if (!after) return 'Shift (+1)';
  if (after === 'random') return 'Random';
  if (typeof after === 'object' && 'shift' in after) {
    const offset = Number(after.shift);
    const sign = offset >= 0 ? `+${offset}` : `${offset}`;
    return `Shift (${sign})`;
  }
  return JSON.stringify(after);
}

/** Views the active in-memory playlist and loads or unloads playlists from the SD card. */
export class BledPlaylist extends HTMLElement {
  set device(value) {
    this._device = value;
    if (this.isConnected && value) this.refresh();
  }

  get device() {
    return this._device;
  }

  connectedCallback() {
    this.innerHTML = `
      <section class="card full-width">
        <div class="card-header">
          <div>
            <span class="card-title">Playlist</span>
            <span class="card-subtitle" style="display:block;">View the active effect playlist or load a playlist JSON file from the device SD card.</span>
          </div>
          <button class="btn btn-secondary btn-sm" data-action="refresh">Refresh</button>
        </div>

        <div class="form-row">
          <div class="form-group" style="flex:1; min-width:220px;">
            <label for="playlistPath">Playlist file path</label>
            <input id="playlistPath" type="text" maxlength="64" value="/playlist.json" placeholder="/playlist.json">
          </div>
          <button class="btn btn-primary btn-sm" data-action="load">Load playlist</button>
          <button class="btn btn-secondary btn-sm" data-action="unload">Unload</button>
        </div>
        <div class="form-row">
          <div class="form-group" style="flex:1; min-width:180px;">
            <label for="playlistDirectory">Browse directory</label>
            <input id="playlistDirectory" type="text" value="/" maxlength="64">
          </div>
          <button class="btn btn-secondary btn-sm" data-action="browse">List JSON files</button>
          <span class="card-subtitle" data-role="active">No active playlist loaded.</span>
        </div>
        <div class="dir-table-wrap" data-role="file-list">
          <div class="dir-empty">Browse a directory to find playlist JSON files.</div>
        </div>

        <div class="form-group" style="margin-top: 1rem;">
          <label>Active in-memory playlist (<span data-role="entry-count">0</span> entries)</label>
          <div class="dir-table-wrap" data-role="playlist-entries">
            <div class="dir-empty">No active playlist loaded.</div>
          </div>
        </div>
      </section>`;

    this.pathInput = this.querySelector('#playlistPath');
    this.directoryInput = this.querySelector('#playlistDirectory');
    this.activeLabel = this.querySelector('[data-role="active"]');
    this.fileList = this.querySelector('[data-role="file-list"]');
    this.entryCount = this.querySelector('[data-role="entry-count"]');
    this.entriesContainer = this.querySelector('[data-role="playlist-entries"]');

    this.addEventListener('click', (event) => {
      const action = event.target.closest('[data-action]')?.dataset.action;
      if (!action || !this.device) return;
      const operations = {
        refresh: () => this.refresh(),
        load: () => this.loadPlaylist(),
        unload: () => this.unloadPlaylist(),
        browse: () => this.browsePlaylists(),
      };
      operations[action]?.();
    });

    if (this.device) this.refresh();
  }

  renderEntries(entries = null, activePosition = null) {
    if (!entries) {
      this.entryCount.textContent = '0';
      this.entriesContainer.innerHTML = '<div class="dir-empty">No active playlist loaded.</div>';
      return;
    }

    this.entryCount.textContent = String(entries.length);
    if (entries.length === 0) {
      this.entriesContainer.innerHTML = '<div class="dir-empty">The loaded playlist has no entries.</div>';
      return;
    }

    this.entriesContainer.innerHTML = `
      <table class="dir-table">
        <thead>
          <tr>
            <th scope="col">#</th>
            <th scope="col">Path</th>
            <th scope="col">Duration</th>
            <th scope="col">After</th>
          </tr>
        </thead>
        <tbody>
          ${entries.map((entry, index) => {
            const isActive = activePosition !== null && Number(activePosition) === index;
            return `<tr>
              <td>${index}${isActive ? ' ▶' : ''}</td>
              <td class="dir-row-name" style="cursor:default;">${escapeHtml(entry.path)}</td>
              <td>${escapeHtml(formatDuration(entry.duration))}</td>
              <td>${escapeHtml(formatAfterAction(entry.after))}</td>
            </tr>`;
          }).join('')}
        </tbody>
      </table>`;
  }

  async refresh() {
    if (!this.device) return;
    try {
      const info = await this.device.readActivePlaylist();
      if (!info) {
        this.activeLabel.textContent = 'No active playlist loaded.';
        this.renderEntries(null);
        return;
      }

      if (info.path) {
        this.pathInput.value = info.path;
      }
      const posLabel = info.position != null ? `entry #${info.position}` : 'empty';
      this.activeLabel.textContent = `Active: ${info.path} (${posLabel})`;

      const playlist = await this.device.readPlaylist();
      const entries = Array.isArray(playlist) ? playlist : [];
      this.renderEntries(entries, info.position);
    } catch (error) {
      log(`Could not refresh playlist state: ${error.message}`, 'error');
    }
  }

  async loadPlaylist() {
    const path = this.pathInput.value.trim();
    if (!path) {
      log('Enter a playlist file path before loading a playlist.', 'error');
      return;
    }
    try {
      await this.device.writeActivePlaylist(path);
      await this.refresh();
    } catch (error) {
      log(`Failed to load playlist "${path}": ${error.message}`, 'error');
    }
  }

  async unloadPlaylist() {
    try {
      await this.device.writeActivePlaylist('');
      await this.refresh();
    } catch (error) {
      log(`Failed to unload playlist: ${error.message}`, 'error');
    }
  }

  async browsePlaylists() {
    const directory = this.directoryInput.value.trim() || '/';
    this.fileList.innerHTML = '<div class="dir-empty">Searching…</div>';
    try {
      const entries = await this.device.listDirectory(directory);
      const files = entries.filter((entry) =>
        entry.kind !== 'Directory' && entry.name.toLowerCase().endsWith('.json')
      );
      if (files.length === 0) {
        this.fileList.innerHTML = '<div class="dir-empty">No JSON files found in this directory.</div>';
        return;
      }
      this.fileList.innerHTML = `<ul class="effect-module-list">${files.map((entry) => {
        const path = joinPath(directory, entry.name);
        return `<li><button type="button" class="btn btn-secondary btn-sm" data-playlist="${escapeHtml(path)}">${escapeHtml(path)}</button></li>`;
      }).join('')}</ul>`;
      this.fileList.querySelectorAll('[data-playlist]').forEach((button) => {
        button.addEventListener('click', () => { this.pathInput.value = button.dataset.playlist; });
      });
    } catch (error) {
      this.fileList.innerHTML = `<div class="dir-empty">${escapeHtml(error.message)}</div>`;
      log(`Failed to list JSON files in "${directory}": ${error.message}`, 'error');
    }
  }
}

customElements.define('bled-playlist', BledPlaylist);
