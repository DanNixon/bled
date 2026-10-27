#![no_std]

use bled_effect::guest::prelude::*;

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

impl Effect for SpanPainter {
    fn new(ctx: &mut Context<'_>, fixtures: Fixtures) -> Result<Self, ()> {
        // Set default colour to white
        ctx.parameters_mut()[0..3].fill(255);

        Ok(Self { fixtures })
    }

    fn step_time(&self, _ctx: &Context<'_>) -> u32 {
        100
    }

    fn step(&mut self, ctx: &mut Context<'_>, _timestamp_ms: u32) -> StepResult {
        let command = ctx.command();

        if command[0] == 0 {
            return StepResult::Unchanged;
        }

        let fixture_index = command[1] as usize;
        let span_index = command[2] as usize;

        let parameters = ctx.parameters();
        let red = parameters[0];
        let green = parameters[1];
        let blue = parameters[2];

        let Some(pixel_range) = self
            .fixtures
            .span_pixel_range_at::<4>(fixture_index, span_index)
        else {
            return StepResult::Error;
        };

        ctx.frame_mut().pixels_mut()[pixel_range.start as usize..pixel_range.end as usize]
            .fill(RGB8::new(red, green, blue));

        ctx.command_mut()[0] = 0;

        StepResult::Changed
    }
}
