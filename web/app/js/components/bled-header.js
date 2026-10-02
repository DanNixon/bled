import { devices } from '../services/devices.js';
import { MockBleDevice } from '../services/mock-device.js';
import { log } from '../services/logger.js';

const MOCK_ENABLED = new URLSearchParams(location.search).has('mock');

export class BledHeader extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <header>
        <div class="title-group">
          <h1>BLED</h1>
          <p>Configuration and control of BLED Bluetooth LED controllers</p>
        </div>
        <div class="conn-group">
          <button id="btnConnect" class="btn btn-primary">Connect device</button>
          ${MOCK_ENABLED ? '<button id="btnMock" class="btn btn-secondary">Add mock device</button>' : ''}
        </div>
      </header>
      <div id="noBleAlert" class="alert" style="display:none;">
        <strong>Web Bluetooth Unsupported:</strong> Your current browser does not support the Web Bluetooth API.
        Please use Google Chrome, Microsoft Edge, or a Web Bluetooth capable browser over HTTPS or localhost.
      </div>
    `;

    this.btnConnect = this.querySelector('#btnConnect');
    this.btnMock = this.querySelector('#btnMock');
    this.noBleAlert = this.querySelector('#noBleAlert');

    if (!devices.isSupported) {
      this.noBleAlert.style.display = 'block';
      this.btnConnect.disabled = true;
    }

    this.btnConnect.onclick = async () => {
      try {
        await devices.addDevice();
      } catch (err) {
        // The user dismissing the chooser is not an error worth shouting about.
        if (err?.name === 'NotFoundError') {
          log('No device selected.');
        } else {
          log(`Bluetooth connection failed: ${err.message}`, 'error');
        }
      }
    };

    if (this.btnMock) {
      this.btnMock.onclick = async () => {
        try {
          await devices.connectRaw(MockBleDevice.create(), MockBleDevice);
        } catch (err) {
          log(`Mock connection failed: ${err.message}`, 'error');
        }
      };
    }
  }
}

customElements.define('bled-header', BledHeader);
