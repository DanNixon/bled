# Bluetooth Low Energy (BLE) Pairing

BLED secures its Bluetooth Low Energy emulated serial interface using LE Secure Connections (LESC) passkey authentication and bonding.
A client must authenticate using the device's configured 6 digit passkey before it can send commands.

## Configuration

The pairing passkey is configured via the `ble_passkey` field in `/config.json` on the SD card.
It must be an integer between `0` and `999999` (inclusive):

```json
{
  "name": "bled-default",
  "ble_passkey": 123456,
  ...
}
```

When BLED generates a default `/config.json` on first boot, `ble_passkey` is set to `123456`.

### Bond Storage

Once a host device pairs with BLED, the resulting bond is saved to `/ble_bonds.bin` on the SD card so future connections from that device are encrypted automatically without prompting for the passkey again.

- Up to **9** bonded peers are stored on the SD card.
- To clear all saved bonds on the controller, delete `/ble_bonds.bin` from the SD card and reboot the device.

## Pairing on Android

1. Open the BLED web UI in a Web Bluetooth-capable browser (such as Chrome or Edge) on Android.
2. Tap **Connect BLE** and select your BLED device from the browser picker.
3. When the web UI connects and sends its initial request, Android will automatically display a system Bluetooth pairing prompt.
4. Enter the 6 digit `ble_passkey` and confirm.

Once paired, Android stores the bond in the system Bluetooth settings and will connect without having to enter the passkey again on future visits.

## Pairing on Linux (BlueZ)

On Linux desktops, Chromium-based browsers (Chrome, Edge, Brave, etc.) do not always present a passkey entry dialog during Web Bluetooth connections, and some desktop Bluetooth settings GUIs do not register a passkey-capable default agent for BLE peripherals.
As a result, attempting to pair directly from the browser or desktop GUI may fail immediately without prompting for a passkey.

To pair on Linux, perform a one-time pairing using `bluetoothctl` in a terminal before connecting from the web UI:

1. Close or disconnect any active browser tab attempting to connect to the BLED device.
2. Start `bluetoothctl`:
   ```sh
   bluetoothctl
   ```
3. Register a passkey-capable agent and start scanning:
   ```text
   [bluetooth]# agent KeyboardDisplay
   [bluetooth]# default-agent
   [bluetooth]# scan on
   ```
4. Wait for your BLED device to appear in the scan output (e.g. `[NEW] Device XX:XX:XX:XX:XX:XX bled-default`).
   If you previously attempted to connect to the device and it is cached in BlueZ, remove the stale entry first and let it be rediscovered:
   ```text
   [bluetooth]# remove XX:XX:XX:XX:XX:XX
   ```
5. Pair with the device using its Bluetooth address:
   ```text
   [bluetooth]# pair XX:XX:XX:XX:XX:XX
   ```
6. When prompted by the agent, enter the 6 digit `ble_passkey`:
   ```text
   [agent] Enter passkey (number in 0-999999): 123456
   ```
7. Wait for `Pairing successful`, then disconnect and exit `bluetoothctl`:
   ```text
   [bled-default]# disconnect XX:XX:XX:XX:XX:XX
   [bluetooth]# exit
   ```

Once paired, BlueZ stores the bond for your adapter.
You can now open the BLED web UI in Chrome or Edge, click **Connect BLE**, and connect directly without needing to enter the passkey again.

## Troubleshooting

- **Re-pairing after deleting `/ble_bonds.bin` or reflashing**:
  If the bond is removed from the BLED SD card while your phone or PC still remembers the old bond, remove/"forget" the device in Android's Bluetooth settings (or run `bluetoothctl remove XX:XX:XX:XX:XX:XX` on Linux) and pair again.
- **Connection times out on first pairing attempt**:
  If entering the passkey takes several seconds, the initial configuration read in the web UI may time out while waiting for pairing to finish. Once pairing completes, click **Reload Config** (or reconnect) in the web UI.
