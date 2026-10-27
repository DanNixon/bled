#![no_std]

use bled_effect::guest::prelude::*;

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

impl Effect for SpanColourChase {
    fn new(ctx: &mut Context<'_>, fixtures: Fixtures) -> Result<Self, ()> {
        let delay_bytes = 500u16.to_le_bytes();
        ctx.parameters_mut()[0..2].copy_from_slice(&delay_bytes);
        ctx.parameters_mut()[2] = 0x0F;

        Ok(Self {
            fixtures,
            span_index: 0,
            colour_index: 0,
        })
    }

    fn step_time(&self, ctx: &Context<'_>) -> u32 {
        let delay_bytes = &ctx.parameters()[0..2];
        let delay = u16::from_le_bytes(delay_bytes.try_into().unwrap());
        delay as u32
    }

    fn step(&mut self, ctx: &mut Context<'_>, _timestamp_ms: u32) -> StepResult {
        let brightness = ctx.parameters()[2];
        let colours = [
            (brightness, 0, 0),
            (0, brightness, 0),
            (0, 0, brightness),
            (0, brightness, brightness),
            (brightness, 0, brightness),
            (brightness, brightness, 0),
            (brightness, brightness, brightness),
        ];

        let total_spans = self.fixtures.span_count();
        if total_spans == 0 {
            return StepResult::Unchanged;
        }

        let target_span = self.span_index % total_spans;
        let Some((_, _, span)) = self.fixtures.span_at(target_span) else {
            return StepResult::Error;
        };
        let Some(pixel_range) = self.fixtures.span_pixel_range::<4>(span) else {
            return StepResult::Error;
        };

        let (red, green, blue) = colours[self.colour_index % colours.len()];
        ctx.frame_mut().pixels_mut()[pixel_range.start as usize..pixel_range.end as usize]
            .fill(RGB8::new(red, green, blue));

        self.colour_index = (self.colour_index + 1) % colours.len();
        if self.colour_index == 0 {
            self.span_index = (self.span_index + 1) % total_spans;
        }

        StepResult::Changed
    }
}
