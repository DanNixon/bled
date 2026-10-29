import * as bled from '../../pkg/bled_wasm.js';
import { log } from './logger.js';

const DEVICE_OPERATIONS = [
  'readDeviceInfo', 'rebootDevice', 'statFile', 'readFile', 'writeFile',
  'deleteFile', 'listDirectory', 'setChannelPixels', 'setFixturePixels',
  'setSpanPixels', 'commit', 'commitChannels', 'readBufferInfo', 'readBuffer',
  'readBufferRange', 'writeBuffer', 'readActiveEffect', 'writeActiveEffect',
  'readEffectCommandLength', 'readEffectCommand', 'writeEffectCommand',
  'readEffectParameterLength', 'readEffectParameters', 'writeEffectParameters',
  'readEffectManifest',
];

const WRITE_CHUNK_SIZE = 244;
const toBytes = (value) => (value instanceof Uint8Array ? value : new Uint8Array(value));

export function normaliseConfig(raw) {
  if (!raw) return { name: 'unknown', fixtures: [], channelLengths: [], channelOffsets: [], channels: [] };

  const rawFixtures = raw.fixtures ?? [];
  let maxChannel = -1;
  for (const fixture of rawFixtures) {
    for (const span of fixture.spans ?? []) {
      maxChannel = Math.max(maxChannel, Number(span.channel));
    }
  }

  const channelCount = Math.max(maxChannel + 1, 4);
  const channelLengths = new Array(channelCount).fill(0);
  for (const fixture of rawFixtures) {
    for (const span of fixture.spans ?? []) {
      const channel = Number(span.channel);
      const end = Number(span.start) + Number(span.length);
      if (channel >= 0 && channel < channelCount) {
        channelLengths[channel] = Math.max(channelLengths[channel], end);
      }
    }
  }

  const channelOffsets = new Array(channelCount).fill(0);
  let cursor = 0;
  for (let i = 0; i < channelCount; i++) {
    channelOffsets[i] = cursor;
    cursor += channelLengths[i] * 3;
  }

  const fixtures = rawFixtures.map((fixture, fixtureIndex) => {
    let layoutName = 'Linear';
    if (fixture.layout) {
      if (typeof fixture.layout === 'string') {
        layoutName = fixture.layout.replace('_', ' ');
      } else if (typeof fixture.layout === 'object') {
        layoutName = (Object.keys(fixture.layout)[0] || '').replace('_', ' ');
      }
      layoutName = layoutName.charAt(0).toUpperCase() + layoutName.slice(1);
    }

    const spans = (fixture.spans ?? []).map((span, spanIndex) => {
      const channel = Number(span.channel);
      const start = Number(span.start);
      const length = Number(span.length);
      const byteOffset = (channelOffsets[channel] ?? 0) + start * 3;
      return {
        index: spanIndex,
        name: span.name || `Span ${spanIndex + 1}`,
        channel,
        start,
        length,
        byteOffset,
        byteLength: length * 3,
      };
    });

    const totalPixels = spans.reduce((sum, span) => sum + span.length, 0);
    return {
      index: fixtureIndex,
      name: fixture.name || `Fixture ${fixtureIndex}`,
      layoutName,
      layout: fixture.layout,
      totalPixels,
      totalBytes: totalPixels * 3,
      spans,
    };
  });

  const channels = channelLengths.map((length, index) => ({
    index,
    name: `Channel ${index}`,
    length,
    byteOffset: channelOffsets[index],
    byteLength: length * 3,
  }));

  return {
    name: raw.name || 'unnamed',
    fixtures,
    channelLengths,
    channelOffsets,
    channels,
  };
}

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
    const ranges = fixture.spans
      .filter((fixtureSpan) => fixtureSpan.byteLength > 0)
      .map((fixtureSpan) => ({
        byteOffset: fixtureSpan.byteOffset,
        byteLength: fixtureSpan.byteLength,
      }));
    return {
      startPixel: 0,
      pixelCount: fixture.totalPixels,
      byteLength: ranges.reduce((sum, range) => sum + range.byteLength, 0),
      ranges,
    };
  }

  return { startPixel: 0, pixelCount: 0, byteOffset: 0, byteLength: 0 };
}

export function getChannelBufferRange(channelsOrConfig, channelIndex, segment = null) {
  if (channelsOrConfig && !Array.isArray(channelsOrConfig) && channelsOrConfig.fixtures) {
    return getBufferRange(channelsOrConfig, {
      fixture: channelsOrConfig.fixtures[channelIndex],
      span: segment,
    });
  }

  let channelOffset = 0;
  let targetChannel = null;
  for (const channel of channelsOrConfig ?? []) {
    if (channel.index === channelIndex) {
      targetChannel = channel;
      break;
    }
    channelOffset += channel.length;
  }

  const startPixel = channelOffset + (segment ? segment.start : 0);
  const pixelCount = segment ? segment.length : (targetChannel?.length ?? 0);
  return { startPixel, pixelCount, byteOffset: startPixel * 3, byteLength: pixelCount * 3 };
}

/** One connected BLED device. Emits `ready`, `config` and `disconnected`. */
export class BleDevice extends EventTarget {
  static get isSupported() {
    return !!navigator.bluetooth;
  }

  static async choose() {
    if (!BleDevice.isSupported) {
      throw new Error('Web Bluetooth is not supported in this browser.');
    }
    const service = bled.serial_service_uuid();
    log(`Requesting Bluetooth device for emulated serial service ${service}...`);
    return navigator.bluetooth.requestDevice({ filters: [{ services: [service] }] });
  }

  constructor(device) {
    super();
    this.device = device;
    this.server = null;
    this.chars = {};
    this.rpc = null;
    this.config = null;
    this.configError = null;
    this._queue = Promise.resolve();
    this._rxBuffer = new Uint8Array(0);
    this._rxWaiters = [];
    this._onNotification = (event) => this._receiveBytes(event.target.value);

    for (const name of DEVICE_OPERATIONS) {
      const operation = this[name];
      this[name] = (...args) => this._exclusive(() => operation.apply(this, args));
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

  get displayName() {
    return this.config?.name || this.bleName;
  }

  _log(message, level = 'info') {
    log(`[${this.displayName}] ${message}`, level);
  }

  _exclusive(operation) {
    const run = this._queue.then(operation, operation);
    this._queue = run.catch(() => {});
    return run;
  }

  async connect() {
    try {
      this._log('Connecting to Bluetooth GATT server...');
      this.server = await this.device.gatt.connect();
      const service = await this.server.getPrimaryService(bled.serial_service_uuid());
      this.chars.rx = await service.getCharacteristic(bled.serial_rx_uuid());
      this.chars.tx = await service.getCharacteristic(bled.serial_tx_uuid());

      this.rpc = new bled.RpcClient(
        (timeoutMs) => this._readFrame(timeoutMs),
        (frame) => this._writeFrame(frame),
        () => this._flushRx(),
      );

      this.chars.tx.addEventListener('characteristicvaluechanged', this._onNotification);
      await this.chars.tx.startNotifications();
      this._log('Connected to emulated serial service.', 'success');
    } catch (error) {
      this._teardown();
      throw error;
    }

    await this.loadConfig();
    this.dispatchEvent(new CustomEvent('ready'));
  }

  disconnect() {
    if (this.device?.gatt?.connected) this.device.gatt.disconnect();
    this._handleDisconnected();
  }

  _teardown() {
    const wasConnected = this.server !== null;
    this.chars.tx?.removeEventListener('characteristicvaluechanged', this._onNotification);
    this.server = null;
    this.chars = {};
    this.rpc = null;
    this._rxBuffer = new Uint8Array(0);
    for (const wake of this._rxWaiters.splice(0)) wake();
    return wasConnected;
  }

  _handleDisconnected() {
    if (this._teardown()) {
      this._log('Bluetooth device disconnected.', 'error');
      this.dispatchEvent(new CustomEvent('disconnected'));
    }
  }

  _receiveBytes(dataView) {
    const bytes = new Uint8Array(dataView.buffer, dataView.byteOffset, dataView.byteLength);
    const merged = new Uint8Array(this._rxBuffer.length + bytes.length);
    merged.set(this._rxBuffer);
    merged.set(bytes, this._rxBuffer.length);
    this._rxBuffer = merged;
    for (const wake of this._rxWaiters.splice(0)) wake();
  }

  async _readFrame(timeoutMs) {
    const deadline = performance.now() + timeoutMs;
    while (true) {
      const delimiter = this._rxBuffer.indexOf(0);
      if (delimiter !== -1) {
        const frame = this._rxBuffer.slice(0, delimiter + 1);
        this._rxBuffer = this._rxBuffer.slice(delimiter + 1);
        return frame;
      }

      const remaining = deadline - performance.now();
      if (remaining <= 0) throw new Error('RPC frame timeout');

      await new Promise((resolve, reject) => {
        const wake = () => {
          clearTimeout(timer);
          resolve();
        };
        const timer = setTimeout(() => {
          this._rxWaiters = this._rxWaiters.filter((waiter) => waiter !== wake);
          reject(new Error('RPC frame timeout'));
        }, remaining);
        this._rxWaiters.push(wake);
      });
    }
  }

  async _writeFrame(frame) {
    const bytes = toBytes(frame);
    for (let offset = 0; offset < bytes.length; offset += WRITE_CHUNK_SIZE) {
      await this.chars.rx.writeValueWithResponse(bytes.subarray(offset, offset + WRITE_CHUNK_SIZE));
    }
  }

  _flushRx() {
    const discarded = this._rxBuffer.length;
    this._rxBuffer = new Uint8Array(0);
    return discarded;
  }

  async loadConfig() {
    this.config = null;
    this.configError = null;
    try {
      const bytes = await this.readConfig();
      this.config = normaliseConfig(bled.decode_device_config(bytes));
      this._log(`Loaded config "${this.config.name}" with ${this.config.fixtures.length} fixture(s).`, 'success');
    } catch (error) {
      this.config = null;
      this.configError = error?.message ?? String(error);
      this._log(`Could not read active config: ${this.configError}`, 'error');
    }
    this.dispatchEvent(new CustomEvent('config'));
  }

  async readDeviceInfo() {
    const info = await this.rpc.read_device_info();
    this._log(`Device Info: rev=${info.git_revision}, boot=${info.boot_reason}, uptime=${info.uptime_ms} ms, cap=${info.led_buffer_capacity} B, channels=${info.channel_count}`, 'success');
    return info;
  }

  async rebootDevice() {
    this._log('Sending reboot request...');
    await this.rpc.reboot();
    this._log('Reboot request sent.', 'success');
    this._handleDisconnected();
  }

  async readActiveEffect() {
    return (await this.rpc.get_active_effect()) ?? '';
  }

  async writeActiveEffect(path) {
    await this.rpc.set_active_effect(path || null);
    this._log(path ? `Loaded effect module "${path}".` : 'Unloaded active effect.', 'success');
  }

  async readEffectManifest() {
    const size = await this.rpc.effect_manifest_stat();
    const bytes = await this._readRpcChunks(
      (offset, length) => this.rpc.effect_manifest_read(offset, length),
      size,
    );
    return bled.decode_effect_manifest(bytes);
  }

  async readEffectCommandLength() {
    return this.rpc.effect_command_stat();
  }

  async readEffectCommand() {
    return toBytes(await this.rpc.effect_command_read()).slice();
  }

  async writeEffectCommand(bytes) {
    await this.rpc.effect_command_write(toBytes(bytes));
  }

  async readEffectParameterLength() {
    return this.rpc.effect_parameter_stat();
  }

  async readEffectParameters() {
    return toBytes(await this.rpc.effect_parameter_read()).slice();
  }

  async writeEffectParameters(bytes) {
    await this.rpc.effect_parameter_write(toBytes(bytes));
  }

  async statConfig() {
    const size = await this.rpc.config_stat();
    this._log(`Active config size: ${size} bytes`, 'success');
    return { size };
  }

  async readConfig(onProgress = null) {
    this._log('Reading active config from device...');
    const { size } = await this.statConfig();
    const bytes = await this._readRpcChunks(
      (offset, length) => this.rpc.config_read(offset, length),
      size,
      onProgress,
    );
    this._log(`Successfully read active config (${bytes.length} bytes).`, 'success');
    return bytes;
  }

  async statFile(path) {
    const size = await this.rpc.file_stat(path);
    this._log(`Stat for "${path}": size = ${size} bytes`, 'success');
    return { size };
  }

  async readFile(path, onProgress = null) {
    this._log(`Reading file "${path}" from device...`);
    const chunks = [];
    let offset = 0;
    while (true) {
      const chunk = toBytes(await this.rpc.file_read(path, offset, bled.max_chunk_size()));
      if (chunk.length === 0) break;
      chunks.push(chunk);
      offset += chunk.length;
      if (onProgress) onProgress(Math.min(95, chunks.length * 10));
      if (chunk.length < bled.max_chunk_size()) break;
    }
    const result = this._joinChunks(chunks);
    if (onProgress) onProgress(100);
    this._log(`Successfully read "${path}" (${result.length} bytes).`, 'success');
    return result;
  }

  async writeFile(path, data, onProgress = null) {
    const bytes = toBytes(data);
    if (bytes.length > 0xffffffff) throw new Error('File is too large for the device API.');
    this._log(`Writing ${bytes.length} bytes to "${path}" on device...`);
    await this.rpc.file_create(path, bytes.length);
    const chunkSize = bled.max_chunk_size();
    const chunkCount = Math.ceil(bytes.length / chunkSize);
    for (let offset = 0, index = 0; offset < bytes.length; offset += chunkSize, index++) {
      await this.rpc.file_write(path, offset, bytes.subarray(offset, offset + chunkSize));
      if (onProgress) onProgress(Math.round(((index + 1) / chunkCount) * 100));
    }
    this._log(`Successfully wrote "${path}" (${bytes.length} bytes).`, 'success');
  }

  async deleteFile(path) {
    await this.rpc.file_delete(path);
    this._log(`Successfully deleted "${path}" on device.`, 'success');
  }

  async listDirectory(path = '/', onProgress = null) {
    this._log(`Listing directory "${path}" on device...`);
    const entries = [];
    for (let index = 0; ; index++) {
      const entry = await this.rpc.read_dir_entry(path || '/', index);
      if (entry == null) break;
      entries.push(entry);
      if (onProgress) onProgress(entries.length);
    }
    this._log(`Found ${entries.length} ${entries.length === 1 ? 'entry' : 'entries'} in "${path}".`, 'success');
    return entries;
  }

  async setSpanPixels(channel, start, count, r, g, b, autoCommit = true) {
    await this._writeSpanPixels(channel, start, count, r, g, b);
    if (autoCommit) await this._commit();
  }

  async _writeSpanPixels(channel, start, count, r, g, b) {
    const channelOffset = this.config?.channelOffsets?.[channel] ?? 0;
    const byteOffset = channelOffset + start * 3;
    const rawData = new Uint8Array(count * 3);
    for (let i = 0; i < count; i++) {
      const index = i * 3;
      rawData[index] = r;
      rawData[index + 1] = g;
      rawData[index + 2] = b;
    }
    await this._writeBufferChunks(byteOffset, rawData);
    this._log(`Updated LED buffer for Ch ${channel}.`, 'success');
  }

  async setFixturePixels(fixtureIndex, start, count, r, g, b, autoCommit = true) {
    const fixture = this.config?.fixtures?.[fixtureIndex];
    if (!fixture) throw new Error(`Fixture index ${fixtureIndex} not found in config`);

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
      await this._writeSpanPixels(span.channel, spanStart, spanCount, r, g, b);
    }
    if (autoCommit) await this._commit();
  }

  async setChannelPixels(channel, start, count, r, g, b, autoCommit = true) {
    await this._writeSpanPixels(channel, start, count, r, g, b);
    if (autoCommit) await this._commit();
  }

  async commit() {
    await this._commit();
  }

  async _commit() {
    this._log('Committing / rendering LED buffer...');
    await this.rpc.commit();
    this._log('Successfully rendered LEDs.', 'success');
  }

  async commitChannels(_channels = []) {
    await this._commit();
  }

  async readBufferInfo() {
    return this.rpc.led_buffer_info();
  }

  async readBuffer(size, onProgress = null) {
    return this._readBufferRangeInternal(0, size, onProgress);
  }

  async readBufferRange(offset, size, onProgress = null) {
    return this._readBufferRangeInternal(offset, size, onProgress);
  }

  async _readBufferRangeInternal(offset, size, onProgress = null) {
    const bytes = await this._readRpcChunks(
      (position, length) => this.rpc.led_buffer_read(position, length),
      size,
      onProgress,
      offset,
    );
    this._log(`Successfully read ${bytes.length} bytes from LED buffer.`, 'success');
    return bytes;
  }

  async writeBuffer(offset, rawData, onProgress = null) {
    const bytes = toBytes(rawData);
    const chunkSize = bled.max_chunk_size();
    const chunkCount = Math.ceil(bytes.length / chunkSize);
    for (let start = 0, index = 0; start < bytes.length; start += chunkSize, index++) {
      await this.rpc.led_buffer_write(offset + start, bytes.subarray(start, start + chunkSize));
      if (onProgress) onProgress(Math.round(((index + 1) / chunkCount) * 100));
    }
  }

  async _writeBufferChunks(offset, bytes) {
    const chunkSize = bled.max_chunk_size();
    for (let start = 0; start < bytes.length; start += chunkSize) {
      await this.rpc.led_buffer_write(offset + start, bytes.subarray(start, start + chunkSize));
    }
  }

  async _readRpcChunks(read, size, onProgress = null, startOffset = 0) {
    const chunks = [];
    let offset = startOffset;
    const end = startOffset + size;
    const maxChunk = bled.max_chunk_size();
    while (offset < end) {
      const wanted = Math.min(maxChunk, end - offset);
      const bytes = toBytes(await read(offset, wanted));
      if (bytes.length === 0) break;
      const slice = bytes.subarray(0, Math.min(bytes.length, end - offset));
      chunks.push(slice);
      offset += slice.length;
      if (onProgress && size > 0) {
        onProgress(Math.min(100, Math.round(((offset - startOffset) / size) * 100)));
      }
    }
    if (onProgress) onProgress(100);
    return this._joinChunks(chunks);
  }

  _joinChunks(chunks) {
    const length = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
    const result = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) {
      result.set(chunk, offset);
      offset += chunk.length;
    }
    return result;
  }
}
