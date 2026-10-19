import { escapeHtml } from '../utils/formatters.js';
import { log } from '../services/logger.js';

const bytesToHex = (bytes) => Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join(' ');

function parseHex(text) {
  const normalized = text.trim().replace(/0x/gi, '').replace(/[\s,:_-]+/g, '');
  if (!normalized) return new Uint8Array(0);
  if (!/^[0-9a-f]+$/i.test(normalized) || normalized.length % 2 !== 0) {
    throw new Error('Enter hexadecimal bytes, for example: 01 02 ff.');
  }
  return Uint8Array.from(normalized.match(/.{2}/g), (byte) => Number.parseInt(byte, 16));
}

function joinPath(dir, name) {
  return `${dir.replace(/\/+$/, '')}/${name.replace(/^\/+/, '')}` || `/${name}`;
}

/** Selects a manifest and edits the active effect's ABI command and parameter buffers. */
export class BledEffect extends HTMLElement {
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
            <span class="card-title">Effects</span>
            <span class="card-subtitle" style="display:block;">Load an effect manifest from the device SD card and exchange its ABI buffers.</span>
          </div>
          <button class="btn btn-secondary btn-sm" data-action="refresh">Refresh</button>
        </div>

        <div class="form-row">
          <div class="form-group" style="flex:1; min-width:220px;">
            <label for="effectPath">Manifest path</label>
            <input id="effectPath" type="text" maxlength="64" placeholder="/effects/demo/manifest.json">
          </div>
          <button class="btn btn-primary btn-sm" data-action="load">Load effect</button>
          <button class="btn btn-secondary btn-sm" data-action="unload">Unload</button>
        </div>
        <div class="form-row">
          <div class="form-group" style="flex:1; min-width:180px;">
            <label for="effectDirectory">Browse directory</label>
            <input id="effectDirectory" type="text" value="/" maxlength="64">
          </div>
          <button class="btn btn-secondary btn-sm" data-action="browse">List manifests</button>
          <span class="card-subtitle" data-role="active">No active effect loaded.</span>
        </div>
        <div class="dir-table-wrap" data-role="manifest-list">
          <div class="dir-empty">Browse a directory to find effect manifests.</div>
        </div>

        <div class="effect-buffers">
          <div class="form-group">
            <label for="effectCommand">Command buffer (<span data-role="command-length">—</span> bytes)</label>
            <textarea id="effectCommand" class="code-editor" spellcheck="false" placeholder="Hex bytes, e.g. 01 02 ff"></textarea>
            <div class="form-row">
              <button class="btn btn-secondary btn-sm" data-action="read-command">Read command buffer</button>
              <button class="btn btn-primary btn-sm" data-action="write-command">Write command buffer</button>
            </div>
          </div>
          <div class="form-group">
            <label for="effectParameters">Parameter buffer (<span data-role="parameter-length">—</span> bytes)</label>
            <textarea id="effectParameters" class="code-editor" spellcheck="false" placeholder="Hex bytes, e.g. 01 02 ff"></textarea>
            <div class="form-row">
              <button class="btn btn-secondary btn-sm" data-action="read-parameters">Read parameter buffer</button>
              <button class="btn btn-primary btn-sm" data-action="write-parameters">Write parameter buffer</button>
            </div>
          </div>
        </div>
      </section>`;

    this.pathInput = this.querySelector('#effectPath');
    this.directoryInput = this.querySelector('#effectDirectory');
    this.commandInput = this.querySelector('#effectCommand');
    this.parametersInput = this.querySelector('#effectParameters');
    this.activeLabel = this.querySelector('[data-role="active"]');
    this.manifestList = this.querySelector('[data-role="manifest-list"]');
    this.commandLength = this.querySelector('[data-role="command-length"]');
    this.parameterLength = this.querySelector('[data-role="parameter-length"]');

    this.addEventListener('click', (event) => {
      const action = event.target.closest('[data-action]')?.dataset.action;
      if (!action || !this.device) return;
      const operations = {
        refresh: () => this.refresh(),
        load: () => this.loadEffect(),
        unload: () => this.unloadEffect(),
        browse: () => this.browseManifests(),
        'read-command': () => this.readBuffer('command'),
        'write-command': () => this.writeBuffer('command'),
        'read-parameters': () => this.readBuffer('parameters'),
        'write-parameters': () => this.writeBuffer('parameters'),
      };
      operations[action]?.();
    });

    if (this.device) this.refresh();
  }

  async refresh() {
    if (!this.device) return;
    try {
      const path = await this.device.readActiveEffect();
      this.pathInput.value = path;
      this.activeLabel.textContent = path ? `Active: ${path}` : 'No active effect loaded.';
      if (!path) {
        this.commandLength.textContent = '—';
        this.parameterLength.textContent = '—';
        this.commandInput.value = '';
        this.parametersInput.value = '';
        return;
      }

      const commandLength = await this.device.readEffectCommandLength();
      const command = await this.device.readEffectCommand();
      const parameterLength = await this.device.readEffectParameterLength();
      const parameters = await this.device.readEffectParameters();
      this.commandLength.textContent = String(commandLength);
      this.parameterLength.textContent = String(parameterLength);
      this.commandInput.value = bytesToHex(command);
      this.parametersInput.value = bytesToHex(parameters);
    } catch (error) {
      log(`Could not refresh effect state: ${error.message}`, 'error');
    }
  }

  async loadEffect() {
    const path = this.pathInput.value.trim();
    if (!path) {
      log('Enter a manifest path before loading an effect.', 'error');
      return;
    }
    try {
      await this.device.writeActiveEffect(path);
      await this.refresh();
    } catch (error) {
      log(`Failed to load effect "${path}": ${error.message}`, 'error');
    }
  }

  async unloadEffect() {
    try {
      await this.device.writeActiveEffect('');
      await this.refresh();
    } catch (error) {
      log(`Failed to unload effect: ${error.message}`, 'error');
    }
  }

  async browseManifests() {
    const directory = this.directoryInput.value.trim() || '/';
    this.manifestList.innerHTML = '<div class="dir-empty">Searching…</div>';
    try {
      const entries = await this.device.listDirectory(directory);
      const manifests = entries.filter((entry) =>
        entry.kind !== 'Directory' && entry.name.toLowerCase().endsWith('.json')
      );
      if (manifests.length === 0) {
        this.manifestList.innerHTML = '<div class="dir-empty">No JSON manifests found in this directory.</div>';
        return;
      }
      this.manifestList.innerHTML = `<ul class="effect-manifest-list">${manifests.map((entry) => {
        const path = joinPath(directory, entry.name);
        return `<li><button type="button" class="btn btn-secondary btn-sm" data-manifest="${escapeHtml(path)}">${escapeHtml(path)}</button></li>`;
      }).join('')}</ul>`;
      this.manifestList.querySelectorAll('[data-manifest]').forEach((button) => {
        button.addEventListener('click', () => { this.pathInput.value = button.dataset.manifest; });
      });
    } catch (error) {
      this.manifestList.innerHTML = `<div class="dir-empty">${escapeHtml(error.message)}</div>`;
      log(`Failed to list effect manifests in "${directory}": ${error.message}`, 'error');
    }
  }

  async readBuffer(kind) {
    try {
      if (kind === 'command') {
        const length = await this.device.readEffectCommandLength();
        const bytes = await this.device.readEffectCommand();
        this.commandLength.textContent = String(length);
        this.commandInput.value = bytesToHex(bytes);
        log(`Read ${bytes.length} command byte(s).`, 'success');
      } else {
        const length = await this.device.readEffectParameterLength();
        const bytes = await this.device.readEffectParameters();
        this.parameterLength.textContent = String(length);
        this.parametersInput.value = bytesToHex(bytes);
        log(`Read ${bytes.length} parameter byte(s).`, 'success');
      }
    } catch (error) {
      log(`Could not read effect ${kind}: ${error.message}`, 'error');
    }
  }

  async writeBuffer(kind) {
    const input = kind === 'command' ? this.commandInput : this.parametersInput;
    const expected = Number((kind === 'command' ? this.commandLength : this.parameterLength).textContent);
    try {
      const bytes = parseHex(input.value);
      if (!Number.isInteger(expected) || bytes.length !== expected) {
        throw new Error(`This effect requires exactly ${expected} byte(s).`);
      }
      if (kind === 'command') await this.device.writeEffectCommand(bytes);
      else await this.device.writeEffectParameters(bytes);
      log(`Wrote ${bytes.length} ${kind} byte(s).`, 'success');
      await this.refresh();
    } catch (error) {
      log(`Could not write effect ${kind}: ${error.message}`, 'error');
    }
  }
}

customElements.define('bled-effect', BledEffect);
