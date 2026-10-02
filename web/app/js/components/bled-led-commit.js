import { log } from '../services/logger.js';
import { escapeHtml } from '../utils/formatters.js';

/** Commit controls for a device. Set `.device` before attaching. */
export class BledLedCommit extends HTMLElement {
  connectedCallback() {
    const channels = this.device.config?.channels ?? [];
    const chips = channels.map((ch) => `
      <label class="channel-chip">
        <input type="checkbox" name="commitChannel" value="${ch.index}">
        ${escapeHtml(ch.name)}
      </label>`).join('');

    this.innerHTML = `
      <div class="card">
        <div class="card-header">
          <div>
            <span class="card-title">Commit Channels</span>
            <span class="card-subtitle" style="display: block;">Render staged buffers to physical LED channels</span>
          </div>
          <div class="form-row" style="gap: 0.25rem;">
            <button id="btnSelectAllChannels" class="btn btn-secondary btn-sm" style="padding: 0.15rem 0.4rem; font-size: 0.7rem;">All</button>
            <button id="btnSelectNoChannels" class="btn btn-secondary btn-sm" style="padding: 0.15rem 0.4rem; font-size: 0.7rem;">None</button>
          </div>
        </div>

        ${channels.length
          ? `<div class="channel-chips">${chips}</div>`
          : '<p class="card-subtitle" style="margin-bottom: 1rem;">No channels available (config could not be read).</p>'}

        <button id="btnCommit" class="btn btn-primary" ${channels.length ? '' : 'disabled'}>Commit Channels</button>
      </div>
    `;

    const boxes = () => this.querySelectorAll('input[name="commitChannel"]');

    this.querySelector('#btnCommit').onclick = async () => {
      const selected = Array.from(this.querySelectorAll('input[name="commitChannel"]:checked'))
        .map(cb => parseInt(cb.value, 10));

      if (selected.length === 0) {
        log("No channels selected for commit.", 'error');
        return;
      }

      try {
        await this.device.commitChannels(selected);
      } catch (err) {
        log(`Commit failed: ${err.message}`, 'error');
      }
    };

    this.querySelector('#btnSelectAllChannels').onclick = () => boxes().forEach(cb => cb.checked = true);
    this.querySelector('#btnSelectNoChannels').onclick = () => boxes().forEach(cb => cb.checked = false);
  }
}

customElements.define('bled-led-commit', BledLedCommit);
