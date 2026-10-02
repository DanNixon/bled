import { log } from '../services/logger.js';

/**
 * Colour picker and range controls for one device. Set `.device` before
 * attaching, then call `setPreset()` whenever a channel or segment is selected.
 */
export class BledLedRange extends HTMLElement {
  connectedCallback() {
    this.innerHTML = `
      <div class="card">
        <div class="card-header">
          <div>
            <span class="card-title">LED Range</span>
            <span id="rangeTarget" class="card-subtitle" style="display: block;"></span>
          </div>
        </div>

        <div class="form-row">
          <div class="form-group" style="flex: 1;">
            <label for="rangeChannel">Channel</label>
            <select id="rangeChannel" disabled></select>
          </div>
          <div class="form-group" style="flex: 1;">
            <label for="rangeStart">Start Pixel</label>
            <input type="number" id="rangeStart" value="0" min="0" max="65535">
          </div>
          <div class="form-group" style="flex: 1;">
            <label for="rangeCount">Count</label>
            <input type="number" id="rangeCount" value="1" min="1" max="65535">
          </div>
        </div>

        <div class="form-group">
          <label>Colour</label>
          <div class="color-picker-container">
            <div id="iroPicker"></div>
            <div class="color-detail-bar">
              <div id="colorPreview" class="color-preview-box" title="Selected colour"></div>
              <div class="hex-input-group" title="Hex code">
                <span class="hex-prefix">#</span>
                <input type="text" id="colorHex" value="FF6400" maxlength="6" spellcheck="false" class="hex-input">
              </div>
              <div class="rgb-group">
                <div class="channel-input-wrap" title="Red (0-255)">
                  <label for="colorR">R</label>
                  <input type="number" id="colorR" min="0" max="255" value="255" class="rgb-num">
                </div>
                <div class="channel-input-wrap" title="Green (0-255)">
                  <label for="colorG">G</label>
                  <input type="number" id="colorG" min="0" max="255" value="100" class="rgb-num">
                </div>
                <div class="channel-input-wrap" title="Blue (0-255)">
                  <label for="colorB">B</label>
                  <input type="number" id="colorB" min="0" max="255" value="0" class="rgb-num">
                </div>
              </div>
            </div>
            <div class="swatches-wrap">
              <div class="swatches">
                <div class="swatch" style="background: #ff0000;" title="Red" data-r="255" data-g="0" data-b="0"></div>
                <div class="swatch" style="background: #00ff00;" title="Green" data-r="0" data-g="255" data-b="0"></div>
                <div class="swatch" style="background: #0000ff;" title="Blue" data-r="0" data-g="0" data-b="255"></div>
                <div class="swatch" style="background: #00ffff;" title="Cyan" data-r="0" data-g="255" data-b="255"></div>
                <div class="swatch" style="background: #ff00ff;" title="Magenta" data-r="255" data-g="0" data-b="255"></div>
                <div class="swatch" style="background: #ffff00;" title="Yellow" data-r="255" data-g="255" data-b="0"></div>
                <div class="swatch" style="background: #ffffff;" title="White" data-r="255" data-g="255" data-b="255"></div>
                <div class="swatch" style="background: #ffa757;" title="Warm White" data-r="255" data-g="167" data-b="87"></div>
                <div class="swatch" style="background: #000000;" title="Off / Black" data-r="0" data-g="0" data-b="0"></div>
              </div>
            </div>
          </div>
        </div>

        <div class="form-row" style="justify-content: space-between; border-top: 1px solid var(--card-border); padding-top: 0.75rem;">
          <label style="display: flex; align-items: center; gap: 0.375rem; cursor: pointer;">
            <input type="checkbox" id="autoCommitCheck" checked> Auto-commit channel on stage
          </label>
          <button id="btnStageRange" class="btn btn-primary">Stage Range</button>
        </div>
      </div>
    `;

    // Elements
    this.rangeTarget = this.querySelector('#rangeTarget');
    this.rangeChannel = this.querySelector('#rangeChannel');
    this.rangeStart = this.querySelector('#rangeStart');
    this.rangeCount = this.querySelector('#rangeCount');
    this.colorPreview = this.querySelector('#colorPreview');
    this.colorHex = this.querySelector('#colorHex');
    this.colorR = this.querySelector('#colorR');
    this.colorG = this.querySelector('#colorG');
    this.colorB = this.querySelector('#colorB');
    this.autoCommitCheck = this.querySelector('#autoCommitCheck');
    this.btnStageRange = this.querySelector('#btnStageRange');

    // Initialize iro.js ColorPicker
    this._initColorPicker();

    // Event listeners
    this.btnStageRange.onclick = async () => {
      const ch = parseInt(this.rangeChannel.value, 10);
      const start = parseInt(this.rangeStart.value, 10);
      const count = parseInt(this.rangeCount.value, 10);
      const r = parseInt(this.colorR.value, 10);
      const g = parseInt(this.colorG.value, 10);
      const b = parseInt(this.colorB.value, 10);

      try {
        await this.device.stageRange(ch, start, count, r, g, b, this.autoCommitCheck.checked);
      } catch (err) {
        log(`Stage range failed: ${err.message}`, 'error');
      }
    };

    // Channel options come from the device config, if it could be read.
    for (const ch of this.device.config?.channels ?? []) {
      const opt = document.createElement('option');
      opt.value = String(ch.index);
      opt.textContent = ch.name;
      this.rangeChannel.appendChild(opt);
    }

    if (this._pendingPreset) this.setPreset(this._pendingPreset);
  }

  /**
   * Preset the range controls.
   * `channel` is a config channel `{ index, name, length }`, `segment` an
   * optional `{ name, start, length }`. Without a segment the whole channel is used.
   */
  setPreset({ channel, segment = null }) {
    if (!this.rangeChannel) {
      this._pendingPreset = { channel, segment };
      return;
    }
    const start = segment ? segment.start : 0;
    const count = segment ? segment.length : channel.length;

    this.rangeChannel.value = String(channel.index);
    this.rangeStart.value = start;
    this.rangeCount.value = Math.max(1, count);
    this.rangeTarget.textContent = segment
      ? `${channel.name} / ${segment.name}: pixels ${start}-${start + count - 1}`
      : `${channel.name}: entire channel (${channel.length} pixels)`;
    this.btnStageRange.disabled = count < 1;
  }

  _initColorPicker() {
    if (typeof iro === 'undefined') {
      console.warn("iro.js is not loaded.");
      return;
    }

    const pickerContainer = this.querySelector("#iroPicker");
    const colorPicker = new iro.ColorPicker(pickerContainer, {
      width: 220,
      color: "#ff6400",
      borderWidth: 1,
      borderColor: "#334155",
      layout: [
        {
          component: iro.ui.Wheel,
          options: { wheelLightness: true }
        },
        {
          component: iro.ui.Slider,
          options: { sliderType: 'value' }
        }
      ]
    });

    let isUpdatingColor = false;

    const updateColorUI = (hex, r, g, b) => {
      this.colorPreview.style.backgroundColor = hex;
      this.colorHex.value = hex.replace('#', '').toUpperCase();
      this.colorR.value = r;
      this.colorG.value = g;
      this.colorB.value = b;
    };

    colorPicker.on(['color:init', 'color:change'], (color) => {
      if (isUpdatingColor) return;
      isUpdatingColor = true;
      updateColorUI(color.hexString, color.rgb.r, color.rgb.g, color.rgb.b);
      isUpdatingColor = false;
    });

    colorPicker.on('input:start', () => {
      if (colorPicker.color.value === 0) {
        colorPicker.color.value = 100;
      }
    });

    const onRgbInputsChanged = () => {
      if (isUpdatingColor) return;
      const r = Math.max(0, Math.min(255, parseInt(this.colorR.value, 10) || 0));
      const g = Math.max(0, Math.min(255, parseInt(this.colorG.value, 10) || 0));
      const b = Math.max(0, Math.min(255, parseInt(this.colorB.value, 10) || 0));
      isUpdatingColor = true;
      colorPicker.color.set({ r, g, b });
      this.colorPreview.style.backgroundColor = colorPicker.color.hexString;
      this.colorHex.value = colorPicker.color.hexString.replace('#', '').toUpperCase();
      isUpdatingColor = false;
    };

    this.colorR.addEventListener('input', onRgbInputsChanged);
    this.colorG.addEventListener('input', onRgbInputsChanged);
    this.colorB.addEventListener('input', onRgbInputsChanged);

    this.colorHex.addEventListener('input', () => {
      if (isUpdatingColor) return;
      let val = this.colorHex.value.trim().replace(/^#/, '');
      if (/^[0-9a-fA-F]{6}$/.test(val)) {
        isUpdatingColor = true;
        colorPicker.color.set('#' + val);
        this.colorPreview.style.backgroundColor = colorPicker.color.hexString;
        this.colorR.value = colorPicker.color.rgb.r;
        this.colorG.value = colorPicker.color.rgb.g;
        this.colorB.value = colorPicker.color.rgb.b;
        isUpdatingColor = false;
      }
    });

    this.querySelectorAll('.swatch').forEach(sw => {
      sw.onclick = () => {
        const r = parseInt(sw.dataset.r, 10);
        const g = parseInt(sw.dataset.g, 10);
        const b = parseInt(sw.dataset.b, 10);
        isUpdatingColor = true;
        colorPicker.color.set({ r, g, b });
        updateColorUI(colorPicker.color.hexString, r, g, b);
        isUpdatingColor = false;
      };
    });
  }
}

customElements.define('bled-led-range', BledLedRange);
