#![no_std]
#![allow(static_mut_refs)]

use bled_core::{config::Fixtures, effect::abi::export::StepResult, wasm_effect};

wasm_effect!(
    br#"{
        "name": "Span Col. Chase",
        "command_descriptors": [
        ],
        "parameter_descriptors": [
            { "name": "Delay", "start": 0, "end": 15 },
            { "name": "Brightness", "start": 16, "end": 23 }
        ]
    }"#,
    SpanColourChase,
    4096,
    0,
    3
);

struct SpanColourChase {
    fixtures: Fixtures,
    span_index: usize,
    colour_index: usize,
}

impl WasmEffect for SpanColourChase {
    fn new(ctx: &Context, fixtures: Fixtures) -> Result<Self, ()> {
        // Set default parameters
        let delay_bytes = 500u16.to_le_bytes();
        ctx.parameters()[0..2].copy_from_slice(&delay_bytes);
        ctx.parameters()[2] = 0x0F; // Dim enough you can actually look at the LEDs

        Ok(Self {
            fixtures,
            span_index: 0,
            colour_index: 0,
        })
    }

    fn step_time(&self, ctx: &Context) -> u32 {
        let bytes: [u8; 2] = ctx.parameters()[0..2].try_into().unwrap();
        u16::from_le_bytes(bytes).try_into().unwrap()
    }

    fn step(&mut self, ctx: &Context, _timestamp_ms: u32) -> StepResult {
        let brightness = ctx.parameters()[2];
        let colours = [
            (brightness, 0, 0),                   // Red
            (0, brightness, 0),                   // Green
            (0, 0, brightness),                   // Blue
            (0, brightness, brightness),          // Cyan
            (brightness, 0, brightness),          // Magenta
            (brightness, brightness, 0),          // Yellow
            (brightness, brightness, brightness), // White
        ];

        let Some((total_spans, pixel_range)) = ({
            let total_spans: usize = self
                .fixtures
                .as_slice()
                .iter()
                .map(|fixture| fixture.spans().len())
                .sum();
            if total_spans == 0 {
                return StepResult::Unchanged;
            }

            let target_span = self.span_index % total_spans;
            let mut span_position = 0;
            let mut pixel_range = None;

            'fixtures: for fixture in self.fixtures.as_slice() {
                for span in fixture.spans() {
                    if span_position == target_span {
                        pixel_range = self.fixtures.span_pixel_range::<4>(span);
                        break 'fixtures;
                    }
                    span_position += 1;
                }
            }

            pixel_range.map(|range| (total_spans, range))
        }) else {
            return StepResult::Error;
        };

        let (r, g, b) = colours[self.colour_index % colours.len()];
        for pixel in pixel_range {
            let offset = pixel as usize * 3;
            let frame = ctx.frame();
            frame[offset] = r;
            frame[offset + 1] = g;
            frame[offset + 2] = b;
        }

        self.colour_index = (self.colour_index + 1) % colours.len();
        if self.colour_index == 0 {
            self.span_index = (self.span_index + 1) % total_spans;
        }

        StepResult::Changed
    }
}
