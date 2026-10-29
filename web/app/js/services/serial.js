import * as bled from '../../pkg/bled_wasm.js';
import { BleDevice } from './ble.js';
import { log } from './logger.js';

let nextSerialDeviceId = 0;

const createSerialDeviceId = () => {
  const uuid = globalThis.crypto?.randomUUID?.();
  return `webserial:${uuid ?? ++nextSerialDeviceId}`;
};

/** One connected BLED device using USB CDC through the Web Serial API. */
export class SerialDevice extends BleDevice {
  static get isSupported() {
    return !!globalThis.isSecureContext && !!globalThis.navigator?.serial;
  }

  static async choose() {
    if (!SerialDevice.isSupported) {
      throw new Error('Web Serial requires a supported browser in a secure context (HTTPS or localhost).');
    }
    return navigator.serial.requestPort({
      filters: [{ usbVendorId: 0xc0de, usbProductId: 0xcafe }],
    });
  }

  constructor(port) {
    super(null);
    this.port = port;
    this.serialId = createSerialDeviceId();
    this.reader = null;
    this.writer = null;
    this.readTask = null;
    this.closeTask = null;
    this.serialOpened = false;
    this.portOpened = false;
    this.disconnecting = false;
  }

  get id() {
    return this.serialId;
  }

  get isConnected() {
    return this.serialOpened && !!this.port?.readable && !!this.port?.writable;
  }

  get displayName() {
    if (this.config?.name) return this.config.name;
    const info = this.port?.getInfo?.() ?? {};
    const vendor = info.usbVendorId?.toString(16).padStart(4, '0');
    const product = info.usbProductId?.toString(16).padStart(4, '0');
    return vendor && product ? `USB CDC ${vendor}:${product}` : 'USB CDC device';
  }

  async connect() {
    try {
      this._log('Opening USB serial connection...');
      await this.port.open({ baudRate: 115200 });
      this.portOpened = true;
      await this.port.setSignals({ dataTerminalReady: true });
      this.serialOpened = true;
      this.reader = this.port.readable.getReader();
      this.writer = this.port.writable.getWriter();
      this.rpc = new bled.RpcClient(
        (timeoutMs) => this._readFrame(timeoutMs),
        (frame) => this._writeSerialFrame(frame),
        () => this._flushRx(),
      );
      this.disconnecting = false;
      this.readTask = this._pumpReader(this.reader);
      this._log('Connected over USB CDC.', 'success');
    } catch (error) {
      this.disconnecting = true;
      this._teardown();
      await this._closePort();
      throw error;
    }

    await this.loadConfig();
    if (!this.isConnected) throw new Error('USB serial device disconnected while connecting.');
    this.dispatchEvent(new CustomEvent('ready'));
  }

  async disconnect() {
    this.disconnecting = true;
    const wasConnected = this._teardown();
    await this._closePort();
    if (wasConnected) {
      this._log('USB serial device disconnected.', 'error');
      this.dispatchEvent(new CustomEvent('disconnected'));
    }
    this.disconnecting = false;
  }

  _teardown() {
    const wasConnected = this.serialOpened || this.rpc !== null;
    this.serialOpened = false;
    return super._teardown() || wasConnected;
  }

  _handleDisconnected(readerStopped = false) {
    this.disconnecting = true;
    if (this._teardown()) {
      this._log('USB serial device disconnected.', 'error');
      this.dispatchEvent(new CustomEvent('disconnected'));
      void this._closePort(readerStopped);
    }
  }

  async _pumpReader(reader) {
    try {
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        if (value?.length) this._receiveBytes(value);
      }
    } catch (error) {
      if (!this.disconnecting) this._log(`USB serial read failed: ${error?.message ?? error}`, 'error');
    } finally {
      try {
        reader.releaseLock();
      } catch (_) {
        // The stream may already have released the lock while closing.
      }
      if (!this.disconnecting) this._handleDisconnected(true);
    }
  }

  async _writeSerialFrame(frame) {
    if (!this.writer) throw new Error('USB serial connection is closed.');
    await this.writer.write(toBytes(frame));
  }

  async _closePort(readerStopped = false) {
    if (this.closeTask) return this.closeTask;
    const reader = this.reader;
    const writer = this.writer;
    const readTask = this.readTask;
    this.closeTask = (async () => {
      try {
        await this.port?.setSignals?.({ dataTerminalReady: false });
      } catch (_) {
        // The device may already have been unplugged.
      }
      if (reader && !readerStopped) {
        try {
          await reader.cancel();
        } catch (_) {
          // The device may have already been unplugged.
        }
      }
      if (!readerStopped) await readTask?.catch(() => {});
      if (reader) {
        try {
          reader.releaseLock();
        } catch (_) {
          // The reader loop may already have released the lock.
        }
      }
      if (writer) {
        try {
          await writer.close();
        } catch (_) {
          // Closing a disconnected device can reject.
        } finally {
          try {
            writer.releaseLock();
          } catch (_) {
            // The stream may already have released the lock while closing.
          }
        }
      }
      if (this.portOpened) {
        try {
          await this.port.close();
        } catch (_) {
          // The port may already have been closed by the browser.
        } finally {
          this.portOpened = false;
        }
      }
      this.reader = null;
      this.writer = null;
      this.readTask = null;
    })();
    try {
      await this.closeTask;
    } finally {
      this.closeTask = null;
    }
  }
}

const toBytes = (value) => (value instanceof Uint8Array ? value : new Uint8Array(value));
