import { logger, clearLog } from '../services/logger.js';

export class BledActivityLog extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="card log-panel">
        <div class="card-header" style="padding-bottom: 0.5rem;">
          <span class="card-title">Activity Log</span>
          <button id="btnClearLog" class="btn btn-secondary btn-sm">Clear Log</button>
        </div>
        <pre id="log"></pre>
      </div>
    `;

    this.logEl = this.querySelector('#log');
    this.btnClearLog = this.querySelector('#btnClearLog');

    this.btnClearLog.onclick = () => {
      clearLog();
    };

    // Replay existing logs or show default
    if (logger.history.length > 0) {
      this.logEl.textContent = logger.history.map(h => h.formatted).join('\n');
    } else {
      this.logEl.textContent = 'Initializing WebAssembly runtime...';
    }

    this._onLog = (e) => {
      if (this.logEl.textContent === 'Initializing WebAssembly runtime...') {
        this.logEl.textContent = e.detail.formatted;
      } else {
        this.logEl.textContent += `\n${e.detail.formatted}`;
      }
      this.logEl.scrollTop = this.logEl.scrollHeight;
    };

    this._onClear = () => {
      this.logEl.textContent = 'Log cleared.';
    };

    logger.addEventListener('log', this._onLog);
    logger.addEventListener('clear', this._onClear);
  }

  disconnectedCallback() {
    logger.removeEventListener('log', this._onLog);
    logger.removeEventListener('clear', this._onClear);
  }
}

customElements.define('bled-activity-log', BledActivityLog);
