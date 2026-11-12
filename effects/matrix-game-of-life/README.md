# Matrix Game of Life

Runs three concurrent instances of Conway's Game of Life displayed on the red, green, and blue colour channels respectively on the first available fixture with a `Matrix2d` layout.

## Initial State

On startup, each game instance is initialized with a randomly generated grid state.

## Parameters

- **Step Interval** (bits 0–15, 2 bytes, little-endian): Step delay in milliseconds between generations.
- **Brightness Red** (bits 16–23, 1 byte): Brightness level for live cells on the red channel.
- **Brightness Green** (bits 24–31, 1 byte): Brightness level for live cells on the green channel.
- **Brightness Blue** (bits 32–39, 1 byte): Brightness level for live cells on the blue channel.

## Commands

A single byte interpreted as a bitflag set:
- **Bit 0 (0x01)**: Reset / regenerate the Red instance.
- **Bit 1 (0x02)**: Reset / regenerate the Green instance.
- **Bit 2 (0x04)**: Reset / regenerate the Blue instance.
