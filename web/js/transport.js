// Hardware transport bridge for Web Bluetooth GATT & Web Serial CDC

const WRITE_CHUNK_SIZE = 244;

const toBytes = (value) => (value instanceof Uint8Array ? value : new Uint8Array(value));

export function isBleSupported() {
  return typeof navigator !== 'undefined' && !!navigator.bluetooth;
}

export function isSerialSupported() {
  return typeof globalThis !== 'undefined' && !!globalThis.isSecureContext && !!globalThis.navigator?.serial;
}

export function isSecureContext() {
  return typeof globalThis !== 'undefined' && !!globalThis.isSecureContext;
}

export async function connectBle(serviceUuid, rxUuid, txUuid, onDisconnected) {
  if (!isBleSupported()) {
    throw new Error('Web Bluetooth is not supported in this browser.');
  }

  const device = await navigator.bluetooth.requestDevice({
    filters: [{ services: [serviceUuid] }],
  });

  const server = await device.gatt.connect();
  const service = await server.getPrimaryService(serviceUuid);
  const rxChar = await service.getCharacteristic(rxUuid);
  const txChar = await service.getCharacteristic(txUuid);

  let rxBuffer = new Uint8Array(0);
  let rxWaiters = [];

  const receiveBytes = (dataView) => {
    const bytes = new Uint8Array(dataView.buffer, dataView.byteOffset, dataView.byteLength);
    const merged = new Uint8Array(rxBuffer.length + bytes.length);
    merged.set(rxBuffer);
    merged.set(bytes, rxBuffer.length);
    rxBuffer = merged;
    for (const wake of rxWaiters.splice(0)) wake();
  };

  const notificationListener = (event) => receiveBytes(event.target.value);
  txChar.addEventListener('characteristicvaluechanged', notificationListener);
  await txChar.startNotifications();

  let disconnected = false;
  const handleDisconnect = () => {
    if (!disconnected) {
      disconnected = true;
      txChar.removeEventListener('characteristicvaluechanged', notificationListener);
      for (const wake of rxWaiters.splice(0)) wake();
      if (onDisconnected) onDisconnected();
    }
  };

  device.addEventListener('gattserverdisconnected', handleDisconnect);

  return {
    id: () => device.id,
    name: () => device.name || 'unnamed device',
    isBle: () => true,
    async readFrame(timeoutMs) {
      const deadline = performance.now() + timeoutMs;
      while (true) {
        if (disconnected) throw new Error('Device disconnected');
        const delimiter = rxBuffer.indexOf(0);
        if (delimiter !== -1) {
          const frame = rxBuffer.slice(0, delimiter + 1);
          rxBuffer = rxBuffer.slice(delimiter + 1);
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
            rxWaiters = rxWaiters.filter((w) => w !== wake);
            reject(new Error('RPC frame timeout'));
          }, remaining);
          rxWaiters.push(wake);
        });
      }
    },
    async writeFrame(frame) {
      if (disconnected) throw new Error('Device disconnected');
      const bytes = toBytes(frame);
      for (let offset = 0; offset < bytes.length; offset += WRITE_CHUNK_SIZE) {
        await rxChar.writeValueWithResponse(bytes.subarray(offset, offset + WRITE_CHUNK_SIZE));
      }
    },
    flushRx() {
      const count = rxBuffer.length;
      rxBuffer = new Uint8Array(0);
      return count;
    },
    disconnect() {
      if (!disconnected) {
        if (device.gatt?.connected) device.gatt.disconnect();
        handleDisconnect();
      }
    },
  };
}

let nextSerialId = 0;

export async function connectSerial(vendorId, productId, onDisconnected) {
  if (!isSerialSupported()) {
    throw new Error('Web Serial requires a supported browser in a secure context (HTTPS or localhost).');
  }

  const port = await navigator.serial.requestPort({
    filters: [{ usbVendorId: vendorId, usbProductId: productId }],
  });

  await port.open({ baudRate: 115200 });
  await port.setSignals({ dataTerminalReady: true });

  const reader = port.readable.getReader();
  const writer = port.writable.getWriter();

  let rxBuffer = new Uint8Array(0);
  let rxWaiters = [];
  let disconnected = false;

  const receiveBytes = (bytes) => {
    const merged = new Uint8Array(rxBuffer.length + bytes.length);
    merged.set(rxBuffer);
    merged.set(bytes, rxBuffer.length);
    rxBuffer = merged;
    for (const wake of rxWaiters.splice(0)) wake();
  };

  const handleDisconnect = () => {
    if (!disconnected) {
      disconnected = true;
      for (const wake of rxWaiters.splice(0)) wake();
      if (onDisconnected) onDisconnected();
    }
  };

  (async () => {
    try {
      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        if (value?.length) receiveBytes(value);
      }
    } catch (_) {
    } finally {
      try { reader.releaseLock(); } catch (_) {}
      handleDisconnect();
    }
  })();

  const portInfo = port.getInfo?.() ?? {};
  const v = portInfo.usbVendorId?.toString(16).padStart(4, '0');
  const p = portInfo.usbProductId?.toString(16).padStart(4, '0');
  const name = v && p ? `USB CDC ${v}:${p}` : 'USB CDC device';
  const id = `webserial:${++nextSerialId}`;

  return {
    id: () => id,
    name: () => name,
    isBle: () => false,
    async readFrame(timeoutMs) {
      const deadline = performance.now() + timeoutMs;
      while (true) {
        if (disconnected) throw new Error('Serial device disconnected');
        const delimiter = rxBuffer.indexOf(0);
        if (delimiter !== -1) {
          const frame = rxBuffer.slice(0, delimiter + 1);
          rxBuffer = rxBuffer.slice(delimiter + 1);
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
            rxWaiters = rxWaiters.filter((w) => w !== wake);
            reject(new Error('RPC frame timeout'));
          }, remaining);
          rxWaiters.push(wake);
        });
      }
    },
    async writeFrame(frame) {
      if (disconnected) throw new Error('Serial device disconnected');
      await writer.write(toBytes(frame));
    },
    flushRx() {
      const count = rxBuffer.length;
      rxBuffer = new Uint8Array(0);
      return count;
    },
    async disconnect() {
      if (!disconnected) {
        handleDisconnect();
        try { await port.setSignals({ dataTerminalReady: false }); } catch (_) {}
        try { await reader.cancel(); } catch (_) {}
        try { reader.releaseLock(); } catch (_) {}
        try { await writer.close(); } catch (_) {}
        try { writer.releaseLock(); } catch (_) {}
        try { await port.close(); } catch (_) {}
      }
    },
  };
}

export function saveFileToDisk(data, filename) {
  const blob = new Blob([data], { type: 'application/octet-stream' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
