import { devices } from '../services/devices.js';
import { log } from '../services/logger.js';

export class BledHeader extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <header>
        <div class="title-group">
          <h1>BLED</h1>
          <p>Configuration and control of BLED LED controllers [<a href="https://github.com/DanNixon/bled">GitHub</a>]<span id="gitRevContainer"></span></p>
        </div>
        <div class="conn-group">
          <button id="btnConnectBle" class="btn btn-primary">Connect BLE</button>
          <button id="btnConnectSerial" class="btn btn-primary">Connect USB Serial</button>
        </div>
      </header>
      <div id="transportAlert" class="alert" style="display:none;"></div>
      <div id="transportWarning" class="alert" style="display:none;"></div>
    `;

    this.btnConnectBle = this.querySelector('#btnConnectBle');
    this.btnConnectSerial = this.querySelector('#btnConnectSerial');
    this.transportAlert = this.querySelector('#transportAlert');
    this.transportWarning = this.querySelector('#transportWarning');
    this.gitRevContainer = this.querySelector('#gitRevContainer');

    if (this._revision) {
      this._renderRevision();
    }

    this.btnConnectBle.disabled = !devices.isBleSupported;
    this.btnConnectSerial.disabled = !devices.isSerialSupported;
    this.transportAlert.style.display = devices.isSupported ? 'none' : 'block';
    this.transportAlert.innerHTML = `
      <strong>No supported connection transport is available.</strong>
      Use a browser with Web Bluetooth or Web Serial over HTTPS or localhost.
    `;
    this.transportWarning.style.display = devices.isSerialSupported ? 'none' : 'block';
    this.transportWarning.textContent = !globalThis.isSecureContext
      ? 'Web Serial requires a secure context. Open this page over HTTPS or localhost.'
      : 'Web Serial is not supported by this browser. Use a browser with Web Serial support.';

    this.btnConnectBle.onclick = async () => {
      try {
        await devices.addDevice();
      } catch (err) {
        // The user dismissing the chooser is not an error worth shouting about.
        if (err?.name === 'NotFoundError') {
          log('No Bluetooth device selected.');
        } else {
          log(`Bluetooth connection failed: ${err.message}`, 'error');
        }
      }
    };

    this.btnConnectSerial.onclick = async () => {
      try {
        await devices.addSerialDevice();
      } catch (err) {
        if (err?.name === 'NotFoundError') {
          log('No serial port selected.');
        } else {
          log(`USB serial connection failed: ${err.message}`, 'error');
        }
      }
    };
  }

  setGitRevision(revision) {
    this._revision = revision;
    if (this.gitRevContainer) {
      this._renderRevision();
    }
  }

  _renderRevision() {
    if (!this.gitRevContainer || !this._revision) return;
    const cleanRev = this._revision.replace(/-modified$/, '');
    this.gitRevContainer.innerHTML = ` [<a href="https://github.com/DanNixon/bled/tree/${encodeURIComponent(cleanRev)}">${this._revision}</a>]`;
  }
}

customElements.define('bled-header', BledHeader);
