import { BleDevice } from './ble.js';

const enc = new TextEncoder();
let counter = 0;

const configJson = (name, channels) => enc.encode(JSON.stringify({ name, channels }));

const section = (name, start, mode) => ({ name, start, mode });
const linear = (length) => ({ LinearArray: { length } });

/** In-memory stand-in for a device, enabled with `?mock` in the page URL. */
export class MockBleDevice extends BleDevice {
  /** Create a raw device descriptor; every other mock lacks a readable config. */
  static create() {
    const n = ++counter;
    return { id: `mock-${n}`, name: `mock-${n}`, mockIndex: n };
  }

  constructor(raw) {
    super(raw);
    this.bootTime = Date.now();
    this.buffer = new Uint8Array(0);
    this.files = new Map();
    this.activeConfig = null;
    if (raw.mockIndex % 2 === 1) {
      const cfg = configJson(`Mock ${raw.mockIndex}`, [
        { name: 'Roof', sections: [
          section('Left', 0, linear(10)),
          section('Right', 10, linear(14)),
        ]},
        { name: 'Desk', sections: [
          section('Onboard', 0, { SinglePixel: {} }),
          section('Strip', 1, linear(30)),
          section('Spare', 31, { Inop: { length: 4 } }),
        ]},
        { name: 'Empty', sections: [] },
      ]);
      this.files.set('/config.json', cfg);
      this.activeConfig = cfg;
    }
  }

  async connect() {
    this.server = { connected: true };
    await this.loadConfig();
    this.dispatchEvent(new CustomEvent('ready'));
  }

  disconnect() {
    this._handleDisconnected();
  }

  async readDeviceInfo() {
    return { git_revision: 'mock0123', boot_reason: 'Normal', uptime_ms: Date.now() - this.bootTime };
  }

  async rebootDevice() {
    this._log('Mock reboot.', 'success');
    this._handleDisconnected();
  }

  async statConfig() {
    if (!this.activeConfig) throw new Error("config not found");
    return { size: this.activeConfig.length };
  }

  async readConfig(onProgress = null) {
    if (!this.activeConfig) throw new Error("config not found");
    onProgress?.(100);
    return this.activeConfig.slice();
  }

  async statFile(path) {
    const f = this.files.get(path);
    if (!f) throw new Error(`file not found: ${path}`);
    return { size: f.length };
  }

  async readFile(path, onProgress = null) {
    const f = this.files.get(path);
    if (!f) throw new Error(`file not found: ${path}`);
    onProgress?.(100);
    return f.slice();
  }

  async writeFile(path, data, onProgress = null) {
    this.files.set(path, data.slice());
    onProgress?.(100);
  }

  async deleteFile(path) {
    if (!this.files.delete(path)) throw new Error(`file not found: ${path}`);
  }

  async stageRange(channel, start, count, r, g, b, autoCommit = true) {
    this._log(`(mock) staged Ch ${channel} start ${start} count ${count} RGB(${r},${g},${b})${autoCommit ? ' + commit' : ''}`, 'success');
  }

  async commitChannels(channels) {
    this._log(`(mock) committed channels [${channels.join(', ')}]`, 'success');
  }

  async readBufferInfo() {
    return { capacity: 4096, size: this.buffer.length };
  }

  async readBuffer(size, onProgress = null) {
    onProgress?.(100);
    return this.buffer.slice(0, size);
  }

  async writeBuffer(offset, data, onProgress = null) {
    const next = new Uint8Array(Math.max(this.buffer.length, offset + data.length));
    next.set(this.buffer);
    next.set(data, offset);
    this.buffer = next;
    onProgress?.(100);
  }
}
