import { devices } from '../services/devices.js';
import { log } from '../services/logger.js';

export class BledHeader extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <header>
        <div class="title-group">
          <h1>BLED</h1>
          <p>Configuration and control of BLED Bluetooth LED controllers [<a href="https://github.com/DanNixon/bled">GitHub</a>]<span id="gitRevContainer"></span></p>
        </div>
        <div class="conn-group">
          <button id="btnConnect" class="btn btn-primary">Connect device</button>
        </div>
      </header>
      <div id="noBleAlert" class="alert" style="display:none;">
        <strong>Web Bluetooth Unsupported:</strong> Your current browser does not support the Web Bluetooth API.
        Please use Google Chrome, Microsoft Edge, or a Web Bluetooth capable browser over HTTPS or localhost.
      </div>
    `;

    this.btnConnect = this.querySelector('#btnConnect');
    this.noBleAlert = this.querySelector('#noBleAlert');
    this.gitRevContainer = this.querySelector('#gitRevContainer');

    if (this._revision) {
      this._renderRevision();
    }

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
