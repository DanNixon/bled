#![no_std]

use bled_effect::guest::prelude::*;

wasm_effect!(
    br#"{
        "name": "Mat. Col. Chase",
        "command_descriptors": [
        ],
        "parameter_descriptors": [
            { "name": "Delay", "start": 0, "end": 15 },
            { "name": "Brightness", "start": 16, "end": 23 }
        ]
    }"#,
    MatrixColourChase,
    4096,
    0,
    3
);

struct MatrixColourChase {
    fixtures: Fixtures,
    colour_index: usize,
    fixture_index: usize,
    column: u32,
    row: u32,
}

impl Effect for MatrixColourChase {
    fn new(ctx: &mut Context<'_>, fixtures: Fixtures) -> Result<Self, ()> {
        let delay_bytes = 100u16.to_le_bytes();
        ctx.parameters_mut()[0..2].copy_from_slice(&delay_bytes);
        ctx.parameters_mut()[2] = 0x0F;

        Ok(Self {
            fixtures,
            colour_index: 0,
            fixture_index: 0,
            column: 0,
            row: 0,
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
            RGB8::new(brightness, 0, 0),                   // Red
            RGB8::new(0, brightness, 0),                   // Green
            RGB8::new(0, 0, brightness),                   // Blue
            RGB8::new(0, brightness, brightness),          // Cyan
            RGB8::new(brightness, 0, brightness),          // Magenta
            RGB8::new(brightness, brightness, 0),          // Yellow
            RGB8::new(brightness, brightness, brightness), // White
            RGB8::new(0, 0, 0),                            // Black
        ];

        let matrix_fixtures: heapless::Vec<(usize, Matrix2d), MAX_FIXTURE_COUNT> = self
            .fixtures
            .as_slice()
            .iter()
            .enumerate()
            .filter_map(|(idx, fixture)| match fixture.layout() {
                PixelLayout::Matrix2d(matrix) => Some((idx, *matrix)),
                _ => None,
            })
            .collect();

        if matrix_fixtures.is_empty() {
            return StepResult::Unchanged;
        }

        let (target_fixture_idx, target_matrix) =
            matrix_fixtures[self.fixture_index % matrix_fixtures.len()];

        let width = *target_matrix.width();
        let height = *target_matrix.height();

        if width == 0 || height == 0 {
            return StepResult::Unchanged;
        }

        let col = self.column % width;
        let row = self.row % height;

        let Some(local_pixel_idx) = target_matrix.pixel_index(Point::new(col as i32, row as i32))
        else {
            return StepResult::Error;
        };

        let Some(global_pixel_idx) = self
            .fixtures
            .fixture_pixel_index::<4>(target_fixture_idx, local_pixel_idx)
        else {
            return StepResult::Error;
        };

        let colour = colours[self.colour_index % colours.len()];
        let pixels = ctx.frame_mut().pixels_mut();
        if global_pixel_idx >= pixels.len() {
            return StepResult::Error;
        }
        pixels[global_pixel_idx] = colour;

        self.row = row + 1;
        if self.row >= height {
            self.row = 0;
            self.column = col + 1;
            if self.column >= width {
                self.column = 0;
                self.fixture_index = (self.fixture_index + 1) % matrix_fixtures.len();
                if self.fixture_index == 0 {
                    self.colour_index = (self.colour_index + 1) % colours.len();
                }
            }
        }

        StepResult::Changed
    }
}
