import { devices } from '../services/devices.js';
import './bled-device-info.js';
import './bled-file-staging.js';
import './bled-led-pixels.js';
import './bled-led-commit.js';
import './bled-led-buffer.js';

const VIEWS = {
  info: 'bled-device-info',
  buffer: 'bled-led-buffer',
  commit: 'bled-led-commit',
  files: 'bled-file-staging',
  pixels: 'bled-led-pixels',
};

/**
 * Shows content for the selected tree item. Views are created lazily, one per
 * device, and hidden rather than destroyed when the selection moves so each
 * keeps its state (staged files, colour, buffer dump, ...).
 */
export class BledMainPanel extends HTMLElement {
  connectedCallback() {
    this.views = new Map(); // `${deviceId}:${view}` -> { el, config }
    this.innerHTML = `
      <p class="panel-placeholder">Connect a device, then select it in the tree.</p>
      <div class="panel-views"></div>`;
    this.placeholder = this.querySelector('.panel-placeholder');
    this.container = this.querySelector('.panel-views');

    this._onSelection = () => this._render();
    this._onDevices = () => {
      this._prune();
      this._render();
    };
    devices.addEventListener('selection-changed', this._onSelection);
    devices.addEventListener('devices-changed', this._onDevices);
    this._render();
  }

  disconnectedCallback() {
    devices.removeEventListener('selection-changed', this._onSelection);
    devices.removeEventListener('devices-changed', this._onDevices);
  }

  /** Drop views for removed devices, and config-driven views whose config changed. */
  _prune() {
    for (const [key, entry] of this.views) {
      const dev = devices.get(entry.deviceId);
      const stale = !dev || (entry.configDriven && entry.config !== dev.config);
      if (stale) {
        entry.el.remove();
        this.views.delete(key);
      }
    }
  }

  _view(dev, name) {
    const key = `${dev.id}:${name}`;
    let entry = this.views.get(key);
    if (!entry) {
      const el = document.createElement(VIEWS[name]);
      el.device = dev;
      el.hidden = true;
      this.container.appendChild(el);
      entry = {
        el,
        deviceId: dev.id,
        config: dev.config,
        configDriven: name === 'pixels' || name === 'commit',
      };
      this.views.set(key, entry);
    }
    return entry.el;
  }

  _render() {
    let sel = devices.selection;
    const dev = devices.selectedDevice;
    let channel = null;
    let segment = null;

    if (sel && dev) {
      if (sel.kind === 'channel' || sel.kind === 'segment') {
        channel = dev.config?.channels[sel.channel] ?? null;
        segment = sel.kind === 'segment' ? channel?.segments[sel.segment] ?? null : null;
        // Config was reloaded and no longer has this item.
        if (!channel || (sel.kind === 'segment' && !segment)) {
          devices.select({ deviceId: dev.id, kind: 'device' });
          return;
        }
      }
    } else {
      sel = null;
    }

    let visible = [];
    if (sel) {
      switch (sel.kind) {
        case 'device':
          visible = [this._view(dev, 'info'), this._view(dev, 'commit'), this._view(dev, 'buffer')];
          break;
        case 'sdcard':
          visible = [this._view(dev, 'files')];
          break;
        default: {
          const pixels = this._view(dev, 'pixels');
          pixels.setPreset({ channel, segment });
          visible = [pixels];
        }
      }
    }

    for (const { el } of this.views.values()) el.hidden = !visible.includes(el);
    this.placeholder.hidden = !!sel;
  }
}

customElements.define('bled-main-panel', BledMainPanel);
