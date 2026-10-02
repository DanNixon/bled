import { log } from '../services/logger.js';
import { formatUptime } from '../utils/formatters.js';

/** Version information for a device. Set `.device` before attaching. */
export class BledDeviceInfo extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="card">
        <div class="card-header">
          <div>
            <span class="card-title">Device</span>
            <span id="deviceName" class="card-subtitle" style="margin-left: 0.5rem;"></span>
          </div>
          <div class="form-row">
            <button id="btnRefreshInfo" class="btn btn-secondary btn-sm">Refresh Info</button>
            <button id="btnReloadConfig" class="btn btn-secondary btn-sm">Reload Config</button>
            <button id="btnResetDevice" class="btn btn-danger btn-sm">Reboot Device</button>
          </div>
        </div>
        <div id="configWarning" class="alert" style="display:none; margin-bottom: 1rem;"></div>
        <div class="info-list">
          <div class="info-box">
            <div class="info-box-label">Git Revision</div>
            <div id="infoGitRev" class="info-box-value">—</div>
          </div>
          <div class="info-box">
            <div class="info-box-label">Boot Reason</div>
            <div id="infoBootReason" class="info-box-value">—</div>
          </div>
          <div class="info-box">
            <div class="info-box-label">Uptime</div>
            <div id="infoUptime" class="info-box-value">—</div>
          </div>
        </div>
      </div>
    `;

    this.deviceName = this.querySelector('#deviceName');
    this.configWarning = this.querySelector('#configWarning');
    this.infoGitRev = this.querySelector('#infoGitRev');
    this.infoBootReason = this.querySelector('#infoBootReason');
    this.infoUptime = this.querySelector('#infoUptime');

    this.querySelector('#btnRefreshInfo').onclick = () => this.refresh();
    this.querySelector('#btnReloadConfig').onclick = () => this.device.loadConfig();
    this.querySelector('#btnResetDevice').onclick = () => this.resetDevice();

    this._onConfig = () => this._renderConfigState();
    this.device.addEventListener('config', this._onConfig);
    this._renderConfigState();
    this.refresh();
  }

  disconnectedCallback() {
    this.device?.removeEventListener('config', this._onConfig);
  }

  _renderConfigState() {
    const { device } = this;
    this.deviceName.textContent = `(${device.displayName})`;
    if (device.configError) {
      this.configWarning.style.display = 'block';
      this.configWarning.textContent =
        `Could not read active config: ${device.configError}. No channels are available until it can be read.`;
    } else {
      this.configWarning.style.display = 'none';
    }
  }

  async refresh() {
    try {
      const info = await this.device.readDeviceInfo();
      this.infoGitRev.textContent = info.git_revision;
      this.infoBootReason.textContent = info.boot_reason;
      this.infoUptime.textContent = formatUptime(info.uptime_ms);
    } catch (err) {
      log(`Failed reading Device Info: ${err.message}`, 'error');
    }
  }

  async resetDevice() {
    if (!confirm(`Are you sure you want to reboot "${this.device.displayName}"?`)) return;
    try {
      await this.device.rebootDevice();
    } catch (err) {
      log(`Reset failed: ${err.message}`, 'error');
    }
  }
}

customElements.define('bled-device-info', BledDeviceInfo);
