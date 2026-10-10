import { BleDevice } from './ble.js';

const enc = new TextEncoder();
let counter = 0;

const fixture = (name, layout, spans) => ({ name, layout, spans });
const span = (channel, start, length) => ({ channel, start, length });

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
      const cfg = enc.encode(
        JSON.stringify({
          name: `Mock ${raw.mockIndex}`,
          fixtures: [
            fixture('Roof', { linear_array: {} }, [
              span(0, 0, 10),
              span(0, 10, 14),
            ]),
            fixture('Onboard', { linear_array: {} }, [
              span(0, 24, 1),
              span(1, 0, 1),
              span(2, 0, 1),
              span(3, 0, 1),
            ]),
            fixture('Desk', { linear_array: {} }, [
              span(1, 1, 30),
            ]),
          ],
        })
      );
      this.files.set('/config.json', cfg);
      this.files.set('/effects.json', enc.encode(JSON.stringify({ effects: ['rainbow', 'chase', 'breathe'] })));
      this.activeConfig = cfg;
    }
  }

  async connect() {
    this.server = { connected: true };
    await this.loadConfig();
    const totalPixels = this.config?.channelLengths?.reduce((sum, len) => sum + len, 0) ?? 0;
    if (this.buffer.length < totalPixels * 3) {
      this.buffer = new Uint8Array(totalPixels * 3);
    }
    this.dispatchEvent(new CustomEvent('ready'));
  }

  disconnect() {
    this._handleDisconnected();
  }

  async readDeviceInfo() {
    return {
      git_revision: 'mock0123',
      boot_reason: 'Normal',
      uptime_ms: Date.now() - this.bootTime,
      led_buffer_capacity: 16384,
      channel_count: 4,
    };
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

  async listDirectory(path = '/', onProgress = null) {
    const cleanPath = (!path || path === '/') ? '/' : (path.endsWith('/') ? path : `${path}/`);
    const entries = [];
    const seen = new Set();

    for (const [filePath, data] of this.files.entries()) {
      let rel = filePath;
      if (cleanPath === '/') {
        if (rel.startsWith('/')) rel = rel.slice(1);
      } else if (filePath.startsWith(cleanPath)) {
        rel = filePath.slice(cleanPath.length);
      } else {
        continue;
      }
      const parts = rel.split('/').filter(Boolean);
      if (parts.length === 0) continue;
      const name = parts[0];
      if (seen.has(name)) continue;
      seen.add(name);

      const is_dir = parts.length > 1;
      const kind = is_dir ? 'Directory' : { File: { size: BigInt(data.length) } };
      entries.push({ name, kind });
    }
    entries.sort((a, b) => {
      const aIsDir = a.kind === 'Directory';
      const bIsDir = b.kind === 'Directory';
      if (aIsDir !== bIsDir) return aIsDir ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    onProgress?.(entries.length);
    this._log(`(mock) listed directory "${path}": ${entries.length} items`, 'success');
    return entries;
  }

  async setSpanPixels(channel, start, count, r, g, b, autoCommit = true) {
    const channelOffset = this.config?.channelOffsets?.[channel] ?? 0;
    const byteOffset = (channelOffset + start) * 3;
    const rawData = new Uint8Array(count * 3);
    for (let i = 0; i < count; i++) {
      const idx = i * 3;
      rawData[idx] = r;
      rawData[idx + 1] = g;
      rawData[idx + 2] = b;
    }
    const next = new Uint8Array(Math.max(this.buffer.length, byteOffset + rawData.length));
    next.set(this.buffer);
    next.set(rawData, byteOffset);
    this.buffer = next;
    this._log(`(mock) set Ch ${channel} start ${start} count ${count} RGB(${r},${g},${b})${autoCommit ? ' + commit' : ''}`, 'success');
  }

  async setChannelPixels(channel, start, count, r, g, b, autoCommit = true) {
    return this.setSpanPixels(channel, start, count, r, g, b, autoCommit);
  }

  async commit() {
    this._log('(mock) committed / rendered LEDs', 'success');
  }

  async commitChannels(_channels = []) {
    return this.commit();
  }

  async readBufferInfo() {
    return { capacity: 16384, size: this.buffer.length };
  }

  async readBuffer(size, onProgress = null) {
    return this.readBufferRange(0, size, onProgress);
  }

  async readBufferRange(offset, size, onProgress = null) {
    onProgress?.(100);
    const result = new Uint8Array(size);
    if (this.buffer.length > offset) {
      const available = this.buffer.subarray(offset, Math.min(this.buffer.length, offset + size));
      result.set(available);
    }
    return result;
  }

  async writeBuffer(offset, data, onProgress = null) {
    const next = new Uint8Array(Math.max(this.buffer.length, offset + data.length));
    next.set(this.buffer);
    next.set(data, offset);
    this.buffer = next;
    onProgress?.(100);
  }
}
