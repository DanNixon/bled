import { BleDevice } from './ble.js';
import { SerialDevice } from './serial.js';
import { log } from './logger.js';

/**
 * Registry of connected devices plus the current tree selection.
 *
 * Selection shape: `{ deviceId, kind, fixture?, span?, channel?, segment? }` where kind is one of
 * 'device' | 'sdcard' | 'effect' | 'fixture' | 'span' | 'channel' | 'segment'.
 */
class DeviceRegistry extends EventTarget {
  constructor() {
    super();
    this.devices = new Map();
    this.selection = null;
    this.pending = 0;
  }

  get isSupported() {
    return this.isBleSupported || this.isSerialSupported;
  }

  get isBleSupported() {
    return BleDevice.isSupported;
  }

  get isSerialSupported() {
    return SerialDevice.isSupported;
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

  /** Ask the user to pick a Bluetooth device and connect to it. */
  async addDevice() {
    return this._addDevice(BleDevice);
  }

  /** Ask the user to pick a serial port and connect to it. */
  async addSerialDevice() {
    return this._addDevice(SerialDevice);
  }

  async _addDevice(DeviceType) {
    const raw = await DeviceType.choose();
    const existing = DeviceType === BleDevice ? this.devices.get(raw.id) : null;
    if (existing) {
      log(`"${existing.displayName}" is already connected.`);
      this.select({ deviceId: existing.id, kind: 'device' });
      return existing;
    }

    const dev = new DeviceType(raw);
    this.pending++;
    this._changed();
    try {
      await dev.connect();
    } finally {
      this.pending--;
      this._changed();
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
