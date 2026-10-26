#![no_std]
#![allow(static_mut_refs)]

use bled_core::{config::Fixtures, effect::abi::export::StepResult, wasm_effect};

wasm_effect!(
    br#"{
        "name": "Span Painter",
        "command_descriptors": [
            { "name": "Draw", "start": 0, "end": 0 },
            { "name": "Fixture", "start": 8, "end": 15 },
            { "name": "Span", "start": 16, "end": 23 }
        ],
        "parameter_descriptors": [
            { "name": "Red", "start": 0, "end": 7 },
            { "name": "Green", "start": 8, "end": 15 },
            { "name": "Blue", "start": 16, "end": 23 }
        ]
    }"#,
    SpanPainter,
    4096,
    3,
    3
);

struct SpanPainter {
    fixtures: Fixtures,
}

impl WasmEffect for SpanPainter {
    fn new(ctx: &Context, fixtures: Fixtures) -> Result<Self, ()> {
        // Set default colour to white
        ctx.parameters()[0..3].fill(255);

        Ok(Self { fixtures })
    }

    fn step_time(&self, _ctx: &Context) -> u32 {
        100
    }

    fn step(&mut self, ctx: &Context, _timestamp_ms: u32) -> StepResult {
        // Return early if no command is set
        let command = ctx.command()[0];
        if command == 0 {
            return StepResult::Unchanged;
        }

        // Get fixture and span indices from command buffer
        let fixture_idx = ctx.command()[1];
        let span_idx = ctx.command()[2];

        // Get colour from parameter buffer
        let (r, g, b) = (
            ctx.parameters()[0],
            ctx.parameters()[1],
            ctx.parameters()[2],
        );

        // Get the range of pixels in the span
        let Some(pixel_range) = ({
            let span =
                self.fixtures.as_slice()[fixture_idx as usize].spans()[span_idx as usize].clone();
            self.fixtures.span_pixel_range::<4>(&span)
        }) else {
            return StepResult::Error;
        };

        // Set pixel colours in frame buffer
        for pixel in pixel_range {
            let pixel = pixel as usize;

            let r_offset = pixel * 3;
            let g_offset = r_offset + 1;
            let b_offset = r_offset + 2;

            ctx.frame()[r_offset] = r;
            ctx.frame()[g_offset] = g;
            ctx.frame()[b_offset] = b;
        }

        // Reset the draw command byte
        ctx.command()[0] = 0;

        StepResult::Changed
    }
}
