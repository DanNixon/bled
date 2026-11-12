#![no_std]

use bled_effect::guest::prelude::*;
use rand::{Rng, SeedableRng, rngs::SmallRng};

const MAX_CELLS: usize = 4096;
const BITSET_WORDS: usize = MAX_CELLS / 64;

const FLAG_RESET_RED: u8 = 1 << 0;
const FLAG_RESET_GREEN: u8 = 1 << 1;
const FLAG_RESET_BLUE: u8 = 1 << 2;

wasm_effect!(
    br#"{
        "name": "Game of Life",
        "command_descriptors": [
            { "name": "Reset Red", "start": 0, "end": 0 },
            { "name": "Reset Green", "start": 1, "end": 1 },
            { "name": "Reset Blue", "start": 2, "end": 2 }
        ],
        "parameter_descriptors": [
            { "name": "Step Interval", "start": 0, "end": 15 },
            { "name": "Brightness Red", "start": 16, "end": 23 },
            { "name": "Brightness Green", "start": 24, "end": 31 },
            { "name": "Brightness Blue", "start": 32, "end": 39 }
        ]
    }"#,
    MatrixGameOfLife,
    4096,
    1,
    5
);

#[derive(Clone, Copy)]
struct LifeGame {
    cells: [u64; BITSET_WORDS],
}

impl LifeGame {
    const fn new() -> Self {
        Self {
            cells: [0; BITSET_WORDS],
        }
    }

    fn get(&self, idx: usize) -> bool {
        if idx >= MAX_CELLS {
            return false;
        }
        (self.cells[idx / 64] & (1 << (idx % 64))) != 0
    }

    fn set(&mut self, idx: usize, val: bool) {
        if idx >= MAX_CELLS {
            return;
        }
        if val {
            self.cells[idx / 64] |= 1 << (idx % 64);
        } else {
            self.cells[idx / 64] &= !(1 << (idx % 64));
        }
    }

    fn randomize(&mut self, rng: &mut SmallRng, count: usize) {
        self.cells = [0; BITSET_WORDS];
        let active_count = count.min(MAX_CELLS);
        for i in 0..active_count {
            self.set(i, rng.random());
        }
    }

    fn step(&self, next: &mut Self, width: u32, height: u32) {
        next.cells = [0; BITSET_WORDS];
        let Some(total_cells) = (width as usize)
            .checked_mul(height as usize)
            .filter(|&total| total <= MAX_CELLS)
        else {
            return;
        };
        let w = width as i32;
        let h = height as i32;

        for y in 0..h {
            for x in 0..w {
                let idx = (y as usize) * (width as usize) + (x as usize);
                if idx >= total_cells {
                    continue;
                }

                let mut live_neighbours = 0;
                for dy in [-1, 0, 1] {
                    for dx in [-1, 0, 1] {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = (x + dx).rem_euclid(w);
                        let ny = (y + dy).rem_euclid(h);
                        let n_idx = (ny as usize) * (width as usize) + (nx as usize);
                        if self.get(n_idx) {
                            live_neighbours += 1;
                        }
                    }
                }

                let alive = self.get(idx);
                let next_alive = match (alive, live_neighbours) {
                    (true, 2) | (true, 3) => true,
                    (false, 3) => true,
                    _ => false,
                };
                next.set(idx, next_alive);
            }
        }
    }
}

fn matrix_cell_count(matrix: &Matrix2d) -> Option<usize> {
    (*matrix.width() as usize)
        .checked_mul(*matrix.height() as usize)
        .filter(|&total| total <= MAX_CELLS)
}

struct MatrixGameOfLife {
    fixtures: Fixtures,
    games: [LifeGame; 3],
    rng: SmallRng,
}

impl Effect for MatrixGameOfLife {
    fn new(ctx: &mut Context<'_>, fixtures: Fixtures) -> Result<Self, ()> {
        let delay_bytes = 500u16.to_le_bytes();
        ctx.parameters_mut()[0..2].copy_from_slice(&delay_bytes);
        ctx.parameters_mut()[2] = 255; // Red brightness
        ctx.parameters_mut()[3] = 255; // Green brightness
        ctx.parameters_mut()[4] = 255; // Blue brightness

        let mut rng = SmallRng::from_seed(ctx.entropy());
        let mut games = [LifeGame::new(); 3];

        let target_matrix = fixtures.as_slice().iter().find_map(|fixture| {
            if let PixelLayout::Matrix2d(matrix) = fixture.layout() {
                Some(*matrix)
            } else {
                None
            }
        });

        let total_cells = match target_matrix {
            Some(ref matrix) => matrix_cell_count(matrix).ok_or(())?,
            None => 0,
        };

        for game in &mut games {
            game.randomize(&mut rng, total_cells);
        }

        Ok(Self {
            fixtures,
            games,
            rng,
        })
    }

    fn step_time(&self, ctx: &Context<'_>) -> u32 {
        let delay_bytes = &ctx.parameters()[0..2];
        let delay = u16::from_le_bytes(delay_bytes.try_into().unwrap());
        (delay as u32).max(1)
    }

    fn step(&mut self, ctx: &mut Context<'_>, _timestamp_ms: u32) -> StepResult {
        let first_matrix =
            self.fixtures
                .as_slice()
                .iter()
                .enumerate()
                .find_map(|(idx, fixture)| match fixture.layout() {
                    PixelLayout::Matrix2d(matrix) => Some((idx, *matrix)),
                    _ => None,
                });

        let Some((target_fixture_idx, target_matrix)) = first_matrix else {
            return StepResult::Unchanged;
        };

        let width = *target_matrix.width();
        let height = *target_matrix.height();

        if width == 0 || height == 0 {
            return StepResult::Unchanged;
        }

        let Some(total_cells) = matrix_cell_count(&target_matrix) else {
            return StepResult::Error;
        };

        let command_byte = ctx.command().first().copied().unwrap_or(0);
        if command_byte != 0 {
            if command_byte & FLAG_RESET_RED != 0 {
                self.games[0].randomize(&mut self.rng, total_cells);
            }
            if command_byte & FLAG_RESET_GREEN != 0 {
                self.games[1].randomize(&mut self.rng, total_cells);
            }
            if command_byte & FLAG_RESET_BLUE != 0 {
                self.games[2].randomize(&mut self.rng, total_cells);
            }
            ctx.command_mut()[0] = 0;
        }

        let mut next_game = LifeGame::new();
        for game in &mut self.games {
            game.step(&mut next_game, width, height);
            *game = next_game;
        }

        let r_bright = ctx.parameters()[2];
        let g_bright = ctx.parameters()[3];
        let b_bright = ctx.parameters()[4];

        let pixels = ctx.frame_mut().pixels_mut();

        for row in 0..height {
            for col in 0..width {
                let cell_idx = (row as usize) * (width as usize) + (col as usize);

                let r = if self.games[0].get(cell_idx) {
                    r_bright
                } else {
                    0
                };
                let g = if self.games[1].get(cell_idx) {
                    g_bright
                } else {
                    0
                };
                let b = if self.games[2].get(cell_idx) {
                    b_bright
                } else {
                    0
                };

                let Some(local_pixel_idx) =
                    target_matrix.pixel_index(Point::new(col as i32, row as i32))
                else {
                    return StepResult::Error;
                };

                let Some(global_pixel_idx) = self
                    .fixtures
                    .fixture_pixel_index::<4>(target_fixture_idx, local_pixel_idx)
                else {
                    return StepResult::Error;
                };

                if global_pixel_idx >= pixels.len() {
                    return StepResult::Error;
                }

                pixels[global_pixel_idx] = RGB8::new(r, g, b);
            }
        }

        StepResult::Changed
    }
}
