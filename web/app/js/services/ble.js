import * as bled from '../../pkg/bled_wasm.js';
import { log } from './logger.js';

/** Operations that talk to the device over GATT; serialised per device. */
const GATT_OPERATIONS = [
  'readDeviceInfo', 'rebootDevice',
  'statFile', 'readFile', 'writeFile', 'deleteFile', 'listDirectory',
  'setChannelPixels', 'setFixturePixels', 'setSpanPixels', 'commit', 'commitChannels',
  'readBufferInfo', 'readBuffer', 'readBufferRange', 'writeBuffer',
];

export const CONFIG_PATH = '/config.json';

const toBytes = (v) => (v instanceof Uint8Array ? v : new Uint8Array(v));

/**
 * Turn the raw decoded DeviceConfig into the shape the UI uses:
 * {
 *   name,
 *   fixtures: [{
 *     index,
 *     name,
 *     layoutName,
 *     layout,
 *     totalPixels,
 *     totalBytes,
 *     byteOffset,
 *     spans: [{ index, channel, start, length, byteOffset, byteLength }]
 *   }],
 *   channelLengths: [number],
 *   channelOffsets: [number],
 *   channels: [{ index, name, length, byteOffset, byteLength }]
 * }
 */
export function normaliseConfig(raw) {
  if (!raw) return { name: 'unknown', fixtures: [], channelLengths: [], channelOffsets: [], channels: [] };

  const rawFixtures = raw.fixtures ?? [];

  // Determine maximum channel index used
  let maxChannel = -1;
  for (const fix of rawFixtures) {
    for (const span of fix.spans ?? []) {
      const ch = Number(span.channel);
      if (ch > maxChannel) maxChannel = ch;
    }
  }
  const channelCount = Math.max(maxChannel + 1, 4);

  // Calculate maximum (start + length) for each channel
  const channelLengths = new Array(channelCount).fill(0);
  for (const fix of rawFixtures) {
    for (const span of fix.spans ?? []) {
      const ch = Number(span.channel);
      const end = Number(span.start) + Number(span.length);
      if (ch >= 0 && ch < channelCount) {
        channelLengths[ch] = Math.max(channelLengths[ch], end);
      }
    }
  }

  // Calculate cumulative channel byte offsets
  const channelOffsets = new Array(channelCount).fill(0);
  let cursor = 0;
  for (let i = 0; i < channelCount; i++) {
    channelOffsets[i] = cursor;
    cursor += channelLengths[i] * 3;
  }

  // Normalise fixtures and spans
  const fixtures = rawFixtures.map((fix, fIndex) => {
    let layoutName = 'Linear';
    if (fix.layout) {
      if (typeof fix.layout === 'string') {
        layoutName = fix.layout.replace('_', ' ');
      } else if (typeof fix.layout === 'object') {
        const key = Object.keys(fix.layout)[0] || '';
        layoutName = key.replace('_', ' ');
      }
      layoutName = layoutName.charAt(0).toUpperCase() + layoutName.slice(1);
    }

    const spans = (fix.spans ?? []).map((span, sIndex) => {
      const ch = Number(span.channel);
      const start = Number(span.start);
      const length = Number(span.length);
      const chOffset = channelOffsets[ch] ?? 0;
      const byteOffset = chOffset + start * 3;
      const byteLength = length * 3;
      return {
        index: sIndex,
        channel: ch,
        start,
        length,
        byteOffset,
        byteLength,
      };
    });

    const totalPixels = spans.reduce((sum, s) => sum + s.length, 0);
    const totalBytes = totalPixels * 3;
    const byteOffset = spans.length > 0 ? spans[0].byteOffset : 0;

    return {
      index: fIndex,
      name: fix.name || `Fixture ${fIndex}`,
      layoutName,
      layout: fix.layout,
      totalPixels,
      totalBytes,
      byteOffset,
      spans,
    };
  });

  // Construct channels array for UI/debugging
  const channels = channelLengths.map((len, ch) => ({
    index: ch,
    name: `Channel ${ch}`,
    length: len,
    byteOffset: channelOffsets[ch],
    byteLength: len * 3,
  }));

  return {
    name: raw.name || 'unnamed',
    fixtures,
    channelLengths,
    channelOffsets,
    channels,
  };
}

/**
 * Calculates start pixel, pixel count, byte offset, and byte length in the
 * device LED buffer for a target fixture or span.
 */
export function getBufferRange(config, target) {
  if (!config || !target) {
    return { startPixel: 0, pixelCount: 0, byteOffset: 0, byteLength: 0 };
  }

  const { fixture, span } = target;
  if (span) {
    return {
      startPixel: span.start,
      pixelCount: span.length,
      byteOffset: span.byteOffset,
      byteLength: span.byteLength,
    };
  }

  if (fixture) {
    return {
      startPixel: 0,
      pixelCount: fixture.totalPixels,
      byteOffset: fixture.byteOffset,
      byteLength: fixture.totalBytes,
      multiSpan: fixture.spans.length > 1,
    };
  }

  return { startPixel: 0, pixelCount: 0, byteOffset: 0, byteLength: 0 };
}

/**
 * Calculates start pixel, pixel count, byte offset, and byte length in the
 * device LED buffer for a channel or segment (backward compatibility helper).
 */
export function getChannelBufferRange(channelsOrConfig, channelIndex, segment = null) {
  if (channelsOrConfig && !Array.isArray(channelsOrConfig) && channelsOrConfig.fixtures) {
    return getBufferRange(channelsOrConfig, {
      fixture: channelsOrConfig.fixtures[channelIndex],
      span: segment,
    });
  }
  let channelOffset = 0;
  let targetChannel = null;
  for (const ch of channelsOrConfig ?? []) {
    if (ch.index === channelIndex) {
      targetChannel = ch;
      break;
    }
    channelOffset += ch.length;
  }
  const startPixel = channelOffset + (segment ? segment.start : 0);
  const pixelCount = segment ? segment.length : (targetChannel?.length ?? 0);
  const byteOffset = startPixel * 3;
  const byteLength = pixelCount * 3;
  return { startPixel, pixelCount, byteOffset, byteLength };
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
      const fixtureCount = this.config.fixtures?.length ?? 0;
      this._log(
        `Loaded config "${this.config.name}" with ${fixtureCount} fixture(s).`,
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
    this._log(`Device Info: rev=${info.git_revision}, boot=${info.boot_reason}, uptime=${info.uptime_ms} ms, cap=${info.led_buffer_capacity} B, channels=${info.channel_count}`, 'success');
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
    startOffset = 0,
    expectedSize = null,
    onProgress = null,
  }) {
    try {
      let offset = startOffset;
      const chunks = [];
      const endOffset = expectedSize !== null ? startOffset + expectedSize : null;

      while (endOffset === null || offset < endOffset) {
        const readCmd = makeReadCmd(offset);
        await controlChar.writeValueWithResponse(readCmd);

        const dataView = await dataChar.readValue();
        const bytes = new Uint8Array(dataView.buffer);
        if (bytes.length === 0) break;

        const remaining = endOffset !== null ? endOffset - offset : bytes.length;
        const slice = remaining < bytes.length ? bytes.subarray(0, remaining) : bytes;
        chunks.push(slice);
        offset += slice.length;

        if (onProgress) {
          if (expectedSize !== null && expectedSize > 0) {
            const bytesRead = offset - startOffset;
            onProgress(Math.min(100, Math.round((bytesRead / expectedSize) * 100)));
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

  async listDirectory(path = '/', onProgress = null) {
    this._log(`Listing directory "${path}" on device...`);
    const entries = [];
    let index = 0;
    try {
      while (true) {
        const readDirCmd = bled.encode_file_io_read_dir(path, index);
        await this.chars.fileControl.writeValueWithResponse(readDirCmd);

        const dataView = await this.chars.fileData.readValue();
        if (dataView.byteLength === 0) {
          break;
        }

        const entry = bled.decode_dir_entry(new Uint8Array(dataView.buffer));
        if (!entry) {
          break;
        }
        entries.push(entry);
        index++;
        if (onProgress) onProgress(entries.length);
      }

      await this.chars.fileControl.writeValueWithResponse(bled.encode_file_io_reset());
      this._log(`Found ${entries.length} ${entries.length === 1 ? 'entry' : 'entries'} in "${path}".`, 'success');
      return entries;
    } catch (err) {
      try { await this.chars.fileControl.writeValueWithResponse(bled.encode_file_io_reset()); } catch (_) {}
      throw err;
    }
  }

  // --- LED Pixels & Commit ---

  async setSpanPixels(channel, start, count, r, g, b, autoCommit = true) {
    const channelOffset = this.config?.channelOffsets?.[channel] ?? 0;
    const byteOffset = channelOffset + start * 3;
    const rawData = new Uint8Array(count * 3);
    for (let i = 0; i < count; i++) {
      const idx = i * 3;
      rawData[idx] = r;
      rawData[idx + 1] = g;
      rawData[idx + 2] = b;
    }

    this._log(`Setting pixels: Ch ${channel}, start ${start}, count ${count}, RGB(${r}, ${g}, ${b}) at buffer offset ${byteOffset}...`);
    const payloadBudget = 244;
    const chunks = bled.slice_led_buffer_write(byteOffset, rawData, payloadBudget);
    await this._writeChunks({
      controlChar: this.chars.bufferControl,
      dataChar: this.chars.bufferData,
      chunks,
      resetCmd: bled.encode_led_buffer_reset(),
    });
    this._log(`Updated LED buffer for Ch ${channel}.`, 'success');

    if (autoCommit) {
      await this.commit();
    }
  }

  async setFixturePixels(fixtureIndex, start, count, r, g, b, autoCommit = true) {
    const fixture = this.config?.fixtures?.[fixtureIndex];
    if (!fixture) {
      throw new Error(`Fixture index ${fixtureIndex} not found in config`);
    }

    // Distribute pixels across the fixture's spans
    let remainingStart = start;
    let remainingCount = count;

    for (const span of fixture.spans) {
      if (remainingCount <= 0) break;
      if (remainingStart >= span.length) {
        remainingStart -= span.length;
        continue;
      }

      const spanStart = span.start + remainingStart;
      const spanCount = Math.min(remainingCount, span.length - remainingStart);
      remainingStart = 0;
      remainingCount -= spanCount;

      await this.setSpanPixels(span.channel, spanStart, spanCount, r, g, b, false);
    }

    if (autoCommit) {
      await this.commit();
    }
  }

  async setChannelPixels(channel, start, count, r, g, b, autoCommit = true) {
    return this.setSpanPixels(channel, start, count, r, g, b, autoCommit);
  }

  async commit() {
    this._log("Committing / rendering LED buffer...");
    const commitBytes = bled.encode_commit();
    await this.chars.commit.writeValueWithResponse(commitBytes);
    this._log("Successfully rendered LEDs.", 'success');
  }

  async commitChannels(_channels = []) {
    return this.commit();
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
    return this._readBufferRangeInternal(0, size, onProgress);
  }

  async readBufferRange(offset, size, onProgress = null) {
    this._log(`Reading LED buffer range: offset ${offset}, ${size} bytes...`);
    return this._readBufferRangeInternal(offset, size, onProgress);
  }

  async _readBufferRangeInternal(offset, size, onProgress = null) {
    const combined = await this._readChunked({
      controlChar: this.chars.bufferControl,
      dataChar: this.chars.bufferData,
      makeReadCmd: (off) => bled.encode_led_buffer_read(off),
      resetCmd: bled.encode_led_buffer_reset(),
      startOffset: offset,
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
