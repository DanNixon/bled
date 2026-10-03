import * as bled from '../../pkg/bled_wasm.js';
import { log } from './logger.js';

/** Operations that talk to the device over GATT; serialised per device. */
const GATT_OPERATIONS = [
  'readDeviceInfo', 'rebootDevice',
  'statFile', 'readFile', 'writeFile', 'deleteFile',
  'stageRange', 'commitChannels',
  'readBufferInfo', 'readBuffer', 'writeBuffer',
];

export const CONFIG_PATH = '/config.json';

const toBytes = (v) => (v instanceof Uint8Array ? v : new Uint8Array(v));

/**
 * Length in pixels of a section, derived from its (externally tagged) mode,
 * e.g. `{ LinearArray: { length: 12 } }`, `{ SinglePixel: {} }`.
 */
export function sectionLength(mode) {
  if (!mode || typeof mode !== 'object') return 0;
  const [variant, body] = Object.entries(mode)[0] ?? [];
  if (variant === 'SinglePixel') return 1;
  const len = body && Number(body.length);
  return Number.isFinite(len) ? len : 0;
}

/**
 * Turn the raw decoded DeviceConfig into the shape the UI uses:
 * `{ name, channels: [{ index, name, length, segments: [{ index, name, start, length }] }] }`
 */
export function normaliseConfig(raw) {
  const channels = (raw.channels ?? []).map((ch, index) => {
    const segments = (ch.sections ?? [])
      .map((s) => ({ name: s.name, start: Number(s.start), length: sectionLength(s.mode) }))
      .filter((s) => s.length > 0)
      .sort((a, b) => a.start - b.start)
      .map((s, i) => ({ ...s, index: i }));
    const length = segments.reduce((end, s) => Math.max(end, s.start + s.length), 0);
    return { index, name: ch.name, length, segments };
  });
  return { name: raw.name, channels };
}

/** One connected BLED device. Emits `ready`, `config` and `disconnected`. */
export class BleDevice extends EventTarget {
  static get isSupported() {
    return !!navigator.bluetooth;
  }

  /** Show the browser's device chooser. Resolves to a raw BluetoothDevice. */
  static async choose() {
    if (!BleDevice.isSupported) {
      throw new Error("Web Bluetooth is not supported in this browser.");
    }
    log(`Requesting Bluetooth device for service ${bled.service_uuid()}...`);
    return navigator.bluetooth.requestDevice({
      filters: [{ services: [bled.service_uuid()] }]
    });
  }

  constructor(device) {
    super();
    this.device = device;
    this.server = null;
    this.chars = {};
    this.config = null;
    this.configError = null;
    this._queue = Promise.resolve();

    // Serialise GATT operations: concurrent operations on a single GATT
    // server are not reliable.
    for (const name of GATT_OPERATIONS) {
      const op = this[name];
      this[name] = (...args) => this._exclusive(() => op.apply(this, args));
    }

    this.device?.addEventListener?.('gattserverdisconnected', () => this._handleDisconnected());
  }

  get id() {
    return this.device.id;
  }

  get isConnected() {
    return !!(this.server && this.server.connected);
  }

  get bleName() {
    return this.device ? (this.device.name || 'unnamed device') : '';
  }

  /** Name from the config file, falling back to the BLE advertised name. */
  get displayName() {
    return this.config?.name || this.bleName;
  }

  _log(msg, level = 'info') {
    log(`[${this.displayName}] ${msg}`, level);
  }

  _exclusive(fn) {
    const run = this._queue.then(fn, fn);
    this._queue = run.catch(() => {});
    return run;
  }

  async connect() {
    try {
      this._log("Connecting to GATT server...");
      this.server = await this.device.gatt.connect();

      this._log("Resolving GATT services and characteristics...");
      const service = await this.server.getPrimaryService(bled.service_uuid());

      this.chars.info          = await service.getCharacteristic(bled.device_info_uuid());
      this.chars.reset         = await service.getCharacteristic(bled.reset_uuid());
      this.chars.configControl = await service.getCharacteristic(bled.config_control_uuid());
      this.chars.configData    = await service.getCharacteristic(bled.config_data_uuid());
      this.chars.commit        = await service.getCharacteristic(bled.commit_uuid());
      this.chars.range         = await service.getCharacteristic(bled.led_range_uuid());
      this.chars.bufferControl = await service.getCharacteristic(bled.led_buffer_control_uuid());
      this.chars.bufferData    = await service.getCharacteristic(bled.led_buffer_data_uuid());
      this.chars.fileControl   = await service.getCharacteristic(bled.file_io_control_uuid());
      this.chars.fileData      = await service.getCharacteristic(bled.file_io_data_uuid());

      this._log("Connected and resolved all characteristics.", 'success');
    } catch (err) {
      this._teardown();
      throw err;
    }
    await this.loadConfig();
    this.dispatchEvent(new CustomEvent('ready'));
  }

  disconnect() {
    if (this.device && this.device.gatt && this.device.gatt.connected) {
      this.device.gatt.disconnect();
    }
    this._handleDisconnected();
  }

  _teardown() {
    const wasConnected = this.server !== null;
    this.server = null;
    this.chars = {};
    return wasConnected;
  }

  _handleDisconnected() {
    if (this._teardown()) {
      this._log("Bluetooth device disconnected.", 'error');
      this.dispatchEvent(new CustomEvent('disconnected'));
    }
  }

  /**
   * Read and decode the device config. Never throws: on failure `config` is
   * null and `configError` describes why, so no channels are shown.
   */
  async loadConfig() {
    this.config = null;
    this.configError = null;
    try {
      const bytes = await this.readConfig();
      this.config = normaliseConfig(bled.decode_device_config(bytes));
      this._log(
        `Loaded config "${this.config.name}" with ${this.config.channels.length} channel(s).`,
        'success'
      );
    } catch (err) {
      this.config = null;
      this.configError = err?.message ?? String(err);
      this._log(`Could not read active config: ${this.configError}`, 'error');
    }
    this.dispatchEvent(new CustomEvent('config'));
  }

  // --- Device Management ---

  async readDeviceInfo() {
    this._log("Reading Device Info...");
    const dataView = await this.chars.info.readValue();
    const rawBytes = new Uint8Array(dataView.buffer);
    const info = bled.decode_device_info(rawBytes);
    this._log(`Device Info: rev=${info.git_revision}, boot=${info.boot_reason}, uptime=${info.uptime_ms} ms`, 'success');
    return info;
  }

  async rebootDevice() {
    this._log("Sending reset command to device...");
    await this.chars.reset.writeValueWithResponse(new Uint8Array([]));
    this._log("Reboot command sent.", 'success');
    this._handleDisconnected();
  }

  // --- Generic Chunked I/O ---

  async _readChunked({
    controlChar,
    dataChar,
    makeReadCmd,
    resetCmd,
    expectedSize = null,
    onProgress = null,
  }) {
    try {
      let offset = 0;
      const chunks = [];

      while (expectedSize === null || offset < expectedSize) {
        const readCmd = makeReadCmd(offset);
        await controlChar.writeValueWithResponse(readCmd);

        const dataView = await dataChar.readValue();
        const bytes = new Uint8Array(dataView.buffer);
        if (bytes.length === 0) break;

        chunks.push(bytes);
        offset += bytes.length;

        if (onProgress) {
          if (expectedSize !== null && expectedSize > 0) {
            onProgress(Math.min(100, Math.round((offset / expectedSize) * 100)));
          } else {
            onProgress(Math.min(95, chunks.length * 15));
          }
        }
      }

      await controlChar.writeValueWithResponse(resetCmd);

      const totalLen = chunks.reduce((acc, c) => acc + c.length, 0);
      const combined = new Uint8Array(totalLen);
      let ptr = 0;
      for (const chunk of chunks) {
        combined.set(chunk, ptr);
        ptr += chunk.length;
      }

      if (onProgress) onProgress(100);
      return combined;
    } catch (err) {
      try {
        await controlChar.writeValueWithResponse(resetCmd);
      } catch (_) {}
      throw err;
    }
  }

  async _writeChunks({
    controlChar,
    dataChar,
    chunks,
    resetCmd,
    onProgress = null,
  }) {
    try {
      for (let i = 0; i < chunks.length; i++) {
        await controlChar.writeValueWithResponse(toBytes(chunks[i].control));
        await dataChar.writeValueWithResponse(toBytes(chunks[i].data));
        if (onProgress) onProgress(Math.round(((i + 1) / chunks.length) * 100));
      }

      await controlChar.writeValueWithResponse(resetCmd);
    } catch (err) {
      try {
        await controlChar.writeValueWithResponse(resetCmd);
      } catch (_) {}
      throw err;
    }
  }

  // --- Config ---

  async statConfig() {
    this._log("Querying stat for active config...");
    try {
      const statCmd = bled.encode_config_stat();
      await this.chars.configControl.writeValueWithResponse(statCmd);

      const dataView = await this.chars.configData.readValue();
      await this.chars.configControl.writeValueWithResponse(bled.encode_config_reset());

      const stat = bled.decode_config_stat(new Uint8Array(dataView.buffer));
      this._log(`Active config stat: size = ${stat.size} bytes`, 'success');
      return stat;
    } catch (err) {
      try { await this.chars.configControl.writeValueWithResponse(bled.encode_config_reset()); } catch (_) {}
      throw err;
    }
  }

  async readConfig(onProgress = null) {
    this._log("Reading active config from device...");
    const stat = await this.statConfig();
    const combined = await this._readChunked({
      controlChar: this.chars.configControl,
      dataChar: this.chars.configData,
      makeReadCmd: (offset) => bled.encode_config_read(offset),
      resetCmd: bled.encode_config_reset(),
      expectedSize: stat.size,
      onProgress,
    });
    this._log(`Successfully read active config (${combined.length} bytes) from device.`, 'success');
    return combined;
  }

  // --- File I/O ---

  async statFile(path) {
    this._log(`Querying stat for remote file "${path}"...`);
    try {
      const statCmd = bled.encode_file_io_stat(path);
      await this.chars.fileControl.writeValueWithResponse(statCmd);

      const dataView = await this.chars.fileData.readValue();
      await this.chars.fileControl.writeValueWithResponse(bled.encode_file_io_reset());

      const stat = bled.decode_file_stat(new Uint8Array(dataView.buffer));
      this._log(`Stat for "${path}": size = ${stat.size} bytes`, 'success');
      return stat;
    } catch (err) {
      try { await this.chars.fileControl.writeValueWithResponse(bled.encode_file_io_reset()); } catch (_) {}
      throw err;
    }
  }

  async readFile(path, onProgress = null) {
    this._log(`Reading file "${path}" from device...`);
    const combined = await this._readChunked({
      controlChar: this.chars.fileControl,
      dataChar: this.chars.fileData,
      makeReadCmd: (offset) => bled.encode_file_io_read(path, offset),
      resetCmd: bled.encode_file_io_reset(),
      onProgress,
    });
    this._log(`Successfully read "${path}" (${combined.length} bytes) from device.`, 'success');
    return combined;
  }

  async writeFile(path, data, onProgress = null) {
    this._log(`Writing ${data.length} bytes to "${path}" on device...`);
    try {
      const createCmd = bled.encode_file_io_create(path, data.length);
      await this.chars.fileControl.writeValueWithResponse(createCmd);
    } catch (err) {
      try { await this.chars.fileControl.writeValueWithResponse(bled.encode_file_io_reset()); } catch (_) {}
      throw err;
    }

    const payloadBudget = 244;
    const chunks = bled.slice_file_write(path, data, payloadBudget);
    this._log(`Sending ${chunks.length} chunks to device...`);

    await this._writeChunks({
      controlChar: this.chars.fileControl,
      dataChar: this.chars.fileData,
      chunks,
      resetCmd: bled.encode_file_io_reset(),
      onProgress,
    });

    this._log(`Successfully wrote "${path}" (${data.length} bytes) to device!`, 'success');
  }

  async deleteFile(path) {
    this._log(`Deleting remote file "${path}"...`);
    const delCmd = bled.encode_file_io_delete(path);
    await this.chars.fileControl.writeValueWithResponse(delCmd);
    this._log(`Successfully deleted "${path}" on device.`, 'success');
  }

  // --- LED Range & Commit ---

  async stageRange(channel, start, count, r, g, b, autoCommit = true) {
    this._log(`Staging range: Ch ${channel}, start ${start}, count ${count}, RGB(${r}, ${g}, ${b})...`);
    const rangeBytes = bled.encode_range(channel, start, count, r, g, b);
    await this.chars.range.writeValueWithResponse(rangeBytes);
    this._log(`Staged range on Ch ${channel}.`, 'success');

    if (autoCommit) {
      const commitBytes = bled.encode_commit(new Uint8Array([channel]));
      await this.chars.commit.writeValueWithResponse(commitBytes);
      this._log(`Rendered/committed channel ${channel}.`, 'success');
    }
  }

  async commitChannels(channels) {
    this._log(`Committing channels [${channels.join(', ')}]...`);
    const commitBytes = bled.encode_commit(new Uint8Array(channels));
    await this.chars.commit.writeValueWithResponse(commitBytes);
    this._log(`Successfully committed channels: ${channels.join(', ')}`, 'success');
  }

  // --- LED Data Buffer ---

  async readBufferInfo() {
    this._log("Reading LED Buffer Info...");
    try {
      const infoCmd = bled.encode_led_buffer_info();
      await this.chars.bufferControl.writeValueWithResponse(infoCmd);

      const dataView = await this.chars.bufferData.readValue();
      await this.chars.bufferControl.writeValueWithResponse(bled.encode_led_buffer_reset());

      const info = bled.decode_led_buffer_info(new Uint8Array(dataView.buffer));
      this._log(`LED Buffer Info: Capacity = ${info.capacity} bytes, Size = ${info.size} bytes`, 'success');
      return info;
    } catch (err) {
      try { await this.chars.bufferControl.writeValueWithResponse(bled.encode_led_buffer_reset()); } catch (_) {}
      throw err;
    }
  }

  async readBuffer(size, onProgress = null) {
    this._log(`Reading entire LED buffer (${size} bytes)...`);
    const combined = await this._readChunked({
      controlChar: this.chars.bufferControl,
      dataChar: this.chars.bufferData,
      makeReadCmd: (offset) => bled.encode_led_buffer_read(offset),
      resetCmd: bled.encode_led_buffer_reset(),
      expectedSize: size,
      onProgress,
    });
    this._log(`Successfully read ${combined.length} bytes from LED buffer!`, 'success');
    return combined;
  }

  async writeBuffer(offset, rawData, onProgress = null) {
    this._log(`Writing ${rawData.length} bytes to LED buffer at offset ${offset}...`);
    const payloadBudget = 244;
    const chunks = bled.slice_led_buffer_write(offset, rawData, payloadBudget);
    this._log(`Slices: ${chunks.length} chunks. Uploading...`);

    await this._writeChunks({
      controlChar: this.chars.bufferControl,
      dataChar: this.chars.bufferData,
      chunks,
      resetCmd: bled.encode_led_buffer_reset(),
      onProgress,
    });

    this._log(`Successfully wrote ${rawData.length} bytes to LED buffer!`, 'success');
  }
}
