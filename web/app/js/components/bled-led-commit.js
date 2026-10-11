import { log } from '../services/logger.js';

/** Commit controls for a device. Set `.device` before attaching. */
export class BledLedCommit extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="card">
        <div class="card-header">
          <div>
            <span class="card-title">Commit / Render</span>
            <span class="card-subtitle" style="display: block;">Render staged buffer to physical LEDs</span>
          </div>
        </div>

        <p class="card-subtitle" style="margin-bottom: 1rem;">
          Flushes the staged LED data buffer to all physical LED outputs.
        </p>

        <button id="btnCommit" class="btn btn-primary">Commit & Render</button>
      </div>
    `;

    this.querySelector('#btnCommit').onclick = async () => {
      try {
        await this.device.commit();
      } catch (err) {
        log(`Commit failed: ${err.message}`, 'error');
      }
    };
  }
}

customElements.define('bled-led-commit', BledLedCommit);
