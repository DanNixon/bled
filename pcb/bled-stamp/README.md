# BLED stamp

A small stamp-like PCB that can be designed onto lights, etc.
Mostly equivalent to the [backpack PCB](../bled-pico-w-backpack), but with a BMS and in a form factor easier to mount onto a larger PCB.

Features:

- All of the features of the backpack PCB
- Single cell lipo battery charging/management
- Some extra IO on the castellations
- USB C (with debug on sideband pins)

## Power

There are three main options for powering an insallation with the stamp:

### 1) USB only

- Do not connect a lipo battery
- Do not connect a 5V supply to "5V IN"
- Power LEDs from "5V OUT"

### 2) battery

- Connect a single cell lipo battery to either the onboard connector or "BATT+", not both
- Do not connect a 5V supply to "5V IN"
- Power LEDs from "5V OUT"

When USB is also connected, LEDs will be powered from VBUS and battery will be recharged.

It is possible to turn off the LED power whilst leaving the MCU powered for low power standby.

### 3) external

- Do not connect a lipo battery
- Connect a 5V supply to "5V IN"
- Power LEDs from the external supply, not "5V OUT"

When USB is connected, logic will be powered from the higher of VBUS or "5V IN".
