import { BleDevice } from './ble.js';
import { log } from './logger.js';

/**
 * Registry of connected devices plus the current tree selection.
 *
 * Selection shape: `{ deviceId, kind, channel?, segment? }` where kind is one of
 * 'device' | 'sdcard' | 'channel' | 'segment'.
 */
class DeviceRegistry extends EventTarget {
  constructor() {
    super();
    this.devices = new Map();
    this.selection = null;
    this.pending = 0;
  }

  get isSupported() {
    return BleDevice.isSupported;
  }

  list() {
    return [...this.devices.values()];
  }

  get(id) {
    return this.devices.get(id);
  }

  get selectedDevice() {
    return this.selection ? this.devices.get(this.selection.deviceId) : undefined;
  }

  /** Ask the user to pick a device and connect to it. Resolves to the BleDevice. */
  async addDevice() {
    const raw = await BleDevice.choose();
    return this.connectRaw(raw);
  }

  /** Connect to an already-chosen device (or a mock) and add it to the tree. */
  async connectRaw(raw, DeviceClass = BleDevice) {
    const existing = this.devices.get(raw.id);
    if (existing) {
      log(`"${existing.displayName}" is already connected.`);
      this.select({ deviceId: existing.id, kind: 'device' });
      return existing;
    }

    const dev = new DeviceClass(raw);
    this.pending++;
    this._changed();
    try {
      await dev.connect();
    } finally {
      this.pending--;
    }

    this.devices.set(dev.id, dev);
    dev.addEventListener('disconnected', () => this.removeDevice(dev.id));
    dev.addEventListener('config', () => this._changed());
    this._changed();
    this.select({ deviceId: dev.id, kind: 'device' });
    return dev;
  }

  removeDevice(id) {
    const dev = this.devices.get(id);
    if (!dev) return;
    this.devices.delete(id);
    if (this.selection?.deviceId === id) {
      this.selection = null;
      this.dispatchEvent(new CustomEvent('selection-changed'));
    }
    this._changed();
  }

  disconnectDevice(id) {
    this.devices.get(id)?.disconnect();
  }

  select(selection) {
    this.selection = selection;
    this.dispatchEvent(new CustomEvent('selection-changed'));
  }

  _changed() {
    this.dispatchEvent(new CustomEvent('devices-changed'));
  }
}

export const devices = new DeviceRegistry();
