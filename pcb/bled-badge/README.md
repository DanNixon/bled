# Badge LED panel

A badge sized LED panel that takes a [BLED Stamp](../bled-stamp).

## Configuration

Load this `fixtures.json` onto the SD card to configure a BLED Stamp for the Badge PCB.

Adjust the `limit` mode as required.
Don't increase the intensity much above the default values: they quickly become blinding when viewed as a badge, generate a lot of heat and will deplete the battery very rapidly (but it would make a very good emergency torch if required).

```json
[
  {
    "name": "onboard",
    "layout": {
      "linear_array": {}
    },
    "spans": [
      {"name": "led", "channel": 0, "start": 0, "length": 1, "limit": {"mode": "clamp", "limit": {"r": 31, "g": 31, "b": 31}}},
      {"name": "led", "channel": 1, "start": 0, "length": 1, "limit": {"mode": "clamp", "limit": {"r": 31, "g": 31, "b": 31}}},
      {"name": "led", "channel": 2, "start": 0, "length": 1, "limit": {"mode": "clamp", "limit": {"r": 31, "g": 31, "b": 31}}},
      {"name": "led", "channel": 3, "start": 0, "length": 1, "limit": {"mode": "clamp", "limit": {"r": 31, "g": 31, "b": 31}}}
    ]
  },
  {
    "name": "badge",
    "layout": {
      "matrix2d": {
        "width": 50,
        "height": 12,
        "origin": "TopLeft",
        "layout": "RowMajor",
        "wiring_order": "Serpentine"
      }
    },
    "spans": [
      {"name": "top",     "channel": 3, "start": 1, "length": 150, "limit": {"mode": "clamp", "limit": {"r": 7, "g": 7, "b": 7}}},
      {"name": "up-mid",  "channel": 1, "start": 1, "length": 150, "limit": {"mode": "clamp", "limit": {"r": 7, "g": 7, "b": 7}}},
      {"name": "low-mid", "channel": 2, "start": 1, "length": 150, "limit": {"mode": "clamp", "limit": {"r": 7, "g": 7, "b": 7}}},
      {"name": "bottom",  "channel": 0, "start": 1, "length": 150, "limit": {"mode": "clamp", "limit": {"r": 7, "g": 7, "b": 7}}}
    ]
  }
]
```
