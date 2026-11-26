//! The empty workspace's (and release notes cover's) Pixel Blast. A coarse field animates the cloud and pointer trail.
//! Ordered dithering turns it into circles batched by the regular JSNIR renderer.
use jsnir::host::{Host, Item, Rect, Rgba};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const SOURCE: &str = "workspace:pixel-blast";
const PAUSED_SOURCE: &str = "workspace:pixel-blast-paused";
const RELEASE_SOURCE: &str = "release:pixel-blast";
// Transparent masks carry the slot through JSNIR's paint list, including its stacking and clip.
const MASK: Rgba = [105, 99, 121, 0];
const PAUSED_MASK: Rgba = [105, 99, 122, 0];
const RELEASE_MASK: Rgba = [105, 99, 123, 0];
const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);
const PITCH: f32 = 7.5;
const DOTS_PER_CELL: usize = 8;
const MAX_DOTS: f32 = 48_000.;
const INTRO_SECONDS: f32 = 1.8;
const HOVER_RADIUS: f32 = 140.;
const RIPPLE_SECONDS: f32 = 2.4;
const MAX_RIPPLES: usize = 10;

// An 8 × 8 Bayer matrix. Nearby dots have different activation thresholds.
const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

pub fn install_masks(host: &mut Host) {
    host.set_image(SOURCE.into(), MASK.to_vec(), [1, 1]);
    host.set_image(PAUSED_SOURCE.into(), PAUSED_MASK.to_vec(), [1, 1]);
    host.set_image(RELEASE_SOURCE.into(), RELEASE_MASK.to_vec(), [1, 1]);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slot {
    rect: Rect,
    radius: [f32; 4],
    clip: Rect,
    base: Rgba,
    accent: Rgba,
    paused: bool,
    release: bool,
}

impl Slot {
    pub(crate) fn from_item(item: &Item) -> Option<Self> {
        let Item::Fill {
            rect,
            radius,
            top,
            bottom,
            clip,
            mask: Some((image, _)),
            ..
        } = item
        else {
            return None;
        };
        let (paused, release) = match image.rgba.as_ref() {
            pixels if pixels == MASK => (false, false),
            pixels if pixels == PAUSED_MASK => (true, false),
            pixels if pixels == RELEASE_MASK => (false, true),
            _ => return None,
        };
        Some(Self {
            rect: *rect,
            radius: *radius,
            clip: *clip,
            base: *top,
            accent: *bottom,
            paused,
            release,
        })
    }

    fn contains(self, [x, y]: [f32; 2]) -> bool {
        [self.rect, self.clip]
            .into_iter()
            .all(|[left, top, width, height]| {
                x >= left && x < left + width && y >= top && y < top + height
            })
    }
}

#[derive(Clone, Copy)]
struct Ripple {
    center: [f32; 2],
    started: f32,
}

#[derive(Default)]
pub struct PixelBlast {
    /// Each instance drives one slot: the workspace backdrop or the release notes cover.
    release: bool,
    slot: Option<Slot>,
    last_tick: Option<Instant>,
    elapsed: f32,
    brush: Option<[f32; 2]>,
    ripples: VecDeque<Ripple>,
    field: Field,
    items: Vec<Item>,
}

impl PixelBlast {
    pub fn release() -> Self {
        Self {
            release: true,
            ..Self::default()
        }
    }

    pub fn press(&mut self, point: [f32; 2]) {
        if self.last_tick.is_none()
            || !self
                .slot
                .is_some_and(|slot| !slot.paused && slot.contains(point))
        {
            return;
        }
        if self.ripples.len() == MAX_RIPPLES {
            self.ripples.pop_front();
        }
        self.ripples.push_back(Ripple {
            center: point,
            started: self.elapsed,
        });
    }

    pub fn pause(&mut self) {
        self.last_tick = None;
    }

    /// Replace the slot in place so menus, labels, and tabs retain JSNIR's paint order.
    pub fn paint(
        &mut self,
        items: &mut Vec<Item>,
        now: Instant,
        pointer: Option<[f32; 2]>,
        animate: bool,
    ) -> Option<Instant> {
        let Some((index, slot)) = items.iter().enumerate().find_map(|(index, item)| {
            Slot::from_item(item)
                .filter(|slot| slot.release == self.release)
                .map(|slot| (index, slot))
        }) else {
            self.slot = None;
            self.last_tick = None;
            self.elapsed = 0.;
            self.brush = None;
            self.ripples.clear();
            self.field.cells.clear();
            self.items.clear();
            return None;
        };

        let running = animate && !slot.paused;
        let geometry_changed = self.slot.is_none_or(|last| last.rect != slot.rect);
        let due = self
            .last_tick
            .is_none_or(|last| now - last >= FRAME_INTERVAL);
        if self.slot != Some(slot) || (running && due) {
            let dt = if running {
                self.last_tick
                    .map_or(0., |last| (now - last).as_secs_f32().min(0.05))
            } else {
                0.
            };
            self.elapsed += dt;
            self.ripples
                .retain(|ripple| self.elapsed - ripple.started < RIPPLE_SECONDS);
            let brush = pointer.filter(|&point| running && slot.contains(point));
            self.brush = brush.map(|[x, y]| {
                let [old_x, old_y] = self.brush.unwrap_or([x, y]);
                let follow = 1. - (-dt / 0.08).exp();
                [lerp(old_x, x, follow), lerp(old_y, y, follow)]
            });
            self.field.update(
                slot.rect,
                self.elapsed,
                dt,
                self.brush,
                &self.ripples,
                geometry_changed,
            );
            self.items.clear();
            self.field.dots(
                |Dot {
                     center,
                     radius,
                     tone,
                     opacity,
                 }| {
                    let diameter = 2. * radius;
                    let mut color = std::array::from_fn(|i| {
                        lerp(f32::from(slot.base[i]), f32::from(slot.accent[i]), tone).round() as u8
                    });
                    color[3] = (f32::from(color[3]) * opacity).round() as u8;
                    if color[3] != 0 {
                        self.items.push(Item::Fill {
                            rect: [center[0] - radius, center[1] - radius, diameter, diameter],
                            radius: [radius; 4],
                            top: color,
                            bottom: color,
                            clip: slot.clip,
                            round_clip: Some((slot.rect, slot.radius)),
                            mask: None,
                        });
                    }
                },
            );
            self.slot = Some(slot);
            if running {
                self.last_tick = Some(now);
            }
        }
        items.splice(index..=index, self.items.iter().cloned());
        if !running {
            self.pause();
        }
        self.last_tick.map(|last| last + FRAME_INTERVAL)
    }
}

#[derive(Clone, Copy, Default)]
struct Cell {
    density: f32,
    trail: f32,
    energy: f32,
}

#[derive(Default)]
struct Field {
    rect: Rect,
    pitch: f32,
    columns: [usize; 2],
    rows: [usize; 2],
    cells: Vec<Cell>,
    elapsed: f32,
}

struct Dot {
    center: [f32; 2],
    radius: f32,
    tone: f32,
    opacity: f32,
}

impl Field {
    fn update(
        &mut self,
        rect: Rect,
        elapsed: f32,
        dt: f32,
        brush: Option<[f32; 2]>,
        ripples: &VecDeque<Ripple>,
        resized: bool,
    ) {
        let [left, top, width, height] = rect;
        // Logical pixels keep dots round at any monitor scale. Bound work on very large windows.
        let pitch = PITCH.max((width * height / MAX_DOTS).sqrt());
        let grid = |side: f32| {
            let dots = (side / pitch).ceil() as usize;
            [dots, dots.div_ceil(DOTS_PER_CELL) + 1]
        };
        self.rect = rect;
        self.pitch = pitch;
        self.columns = grid(width);
        self.rows = grid(height);
        self.elapsed = elapsed;
        if resized {
            self.cells.clear();
        }
        let [_, cols] = self.columns;
        let [_, rows] = self.rows;
        self.cells.resize(cols * rows, Cell::default());

        let wavelength = height.max(1.);
        let step = DOTS_PER_CELL as f32 * pitch;
        let progress = (elapsed / INTRO_SECONDS).min(1.);
        let intro_front = (1. - (1. - progress).powf(2.4)) * (width.hypot(height) * 0.5 + step);
        let trail_decay = (-dt / 0.32).exp();
        for (row, cells) in self.cells.chunks_exact_mut(cols).enumerate() {
            let y = row as f32 * step;
            let dy = y - height * 0.5;
            for (col, cell) in cells.iter_mut().enumerate() {
                let x = col as f32 * step;
                let dx = x - width * 0.5;
                let noise = clouds(dx / wavelength, dy / wavelength, elapsed);
                let mut density = (noise - 0.25) * 1.65;
                let mut energy: f32 = 0.;
                for ripple in ripples {
                    let age = elapsed - ripple.started;
                    let distance = (x + left - ripple.center[0]).hypot(y + top - ripple.center[1])
                        / wavelength;
                    let wave = bell((distance - age * 0.39) / 0.064);
                    let strength = (1. - age / RIPPLE_SECONDS).powi(2) / (1. + distance * 2.8);
                    energy = energy.max(wave * strength);
                }
                if progress < 1. {
                    let delta = dx.hypot(dy) - intro_front;
                    density *= 1. - smooth(-step, step, delta);
                    energy = energy.max(bell(delta / (wavelength * 0.07)) * (1. - progress));
                }
                let hover = brush.map_or(0., |[mx, my]| {
                    let x = (x + left - mx) / HOVER_RADIUS;
                    let y = (y + top - my) / HOVER_RADIUS;
                    (-(x * x + y * y)).exp()
                });
                let trail = hover.max((cell.trail * trail_decay - dt * 0.08).max(0.));
                *cell = Cell {
                    density: density.max(0.) + energy * 0.74 + trail * 0.62,
                    trail,
                    energy,
                };
            }
        }
    }

    fn dots(&self, mut emit: impl FnMut(Dot)) {
        let [left, top, width, height] = self.rect;
        let [nx, cols] = self.columns;
        let [ny, _] = self.rows;
        for row in 0..ny {
            let y = (row as f32 + 0.5) * self.pitch;
            let v = y / height;
            let fade_y = edge_fade(v);
            let cy = row / DOTS_PER_CELL;
            let fy = fraction_in_cell(row);
            let thresholds = &BAYER[row % DOTS_PER_CELL];
            for col in 0..nx {
                let cx = col / DOTS_PER_CELL;
                let fx = fraction_in_cell(col);
                let index = cy * cols + cx;
                let sample = |value: fn(Cell) -> f32| {
                    lerp(
                        lerp(value(self.cells[index]), value(self.cells[index + 1]), fx),
                        lerp(
                            value(self.cells[index + cols]),
                            value(self.cells[index + cols + 1]),
                            fx,
                        ),
                        fy,
                    )
                };
                let density = sample(|c| c.density);
                let threshold = (f32::from(thresholds[col % DOTS_PER_CELL]) + 0.5) / 64.;
                let coverage = smooth(-0.09, 0.09, density - threshold);
                if coverage < 0.035 {
                    continue;
                }
                let x = (col as f32 + 0.5) * self.pitch;
                let u = x / width;
                let energy = sample(|c| c.energy).max(sample(|c| c.trail) * 0.75);
                let tone = (smooth(0.30, 0.95, density) * 0.78).max(energy);
                let quiet_center = 0.52 + 0.48 * smooth(0.10, 0.40, (u - 0.5).hypot(v - 0.5));
                let seed = hash([col as i32, row as i32, 0]);
                let twinkle = 0.5 + 0.5 * (self.elapsed * (1.6 + seed * 2.5) + seed * 37.).sin();
                let opacity = coverage
                    * lerp(0.49, 1., density.min(1.))
                    * (1. - 0.16 * twinkle * twinkle)
                    * quiet_center.max(energy)
                    * fade_y
                    * edge_fade(u);
                let radius = self.pitch
                    * 0.34
                    * coverage.sqrt()
                    * lerp(0.63, 1., density.min(1.))
                    * lerp(0.87, 1.07, seed);
                emit(Dot {
                    center: [left + x, top + y],
                    radius,
                    tone,
                    opacity,
                });
            }
        }
    }
}

fn fraction_in_cell(dot: usize) -> f32 {
    ((dot % DOTS_PER_CELL) as f32 + 0.5) / DOTS_PER_CELL as f32
}

fn edge_fade(position: f32) -> f32 {
    smooth(0., 0.18, position.min(1. - position))
}

fn smooth(low: f32, high: f32, v: f32) -> f32 {
    let t = ((v - low) / (high - low)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn bell(v: f32) -> f32 {
    (-v * v).exp()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn hash([x, y, z]: [i32; 3]) -> f32 {
    let mut n = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    n = (n ^ (n >> 16)).wrapping_mul(0x7feb_352d);
    ((n ^ (n >> 15)) >> 8) as f32 / 16_777_216.
}

fn noise(point: [f32; 3]) -> f32 {
    let low = point.map(|v| v.floor() as i32);
    let [x, y, z] = low;
    let [u, v, w] = std::array::from_fn(|i| smooth(0., 1., point[i] - low[i] as f32));
    let slice = |z| {
        lerp(
            lerp(hash([x, y, z]), hash([x + 1, y, z]), u),
            lerp(hash([x, y + 1, z]), hash([x + 1, y + 1, z]), u),
            v,
        )
    };
    lerp(slice(z), slice(z + 1), w)
}

fn clouds(x: f32, y: f32, elapsed: f32) -> f32 {
    let mut point = [
        x * 2.6 + elapsed * 0.027,
        y * 2.6 - elapsed * 0.014,
        1.8 + elapsed * 0.045,
    ];
    [0.57, 0.27, 0.11, 0.05]
        .into_iter()
        .fold(0., |sum, weight| {
            let layer = noise(point);
            point = point.map(|v| v * 1.72 + 2.37);
            sum + layer * weight
        })
}

/// JSNIR's standalone offscreen capture samples a settled frame through its normal mask path.
pub fn install_capture_mask(host: &mut Host) {
    const WIDTH: usize = 1048;
    const HEIGHT: usize = 792;
    let mut field = Field::default();
    field.update(
        [0., 0., WIDTH as f32, HEIGHT as f32],
        INTRO_SECONDS,
        0.,
        None,
        &VecDeque::new(),
        true,
    );
    let mut rgba = vec![255; WIDTH * HEIGHT * 4];
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 0;
    }
    field.dots(
        |Dot {
             center: [x, y],
             radius,
             opacity,
             ..
         }| {
            let near = radius + 0.5;
            for py in ((y - near) as usize)..((y + near).ceil() as usize).min(HEIGHT) {
                for px in ((x - near) as usize)..((x + near).ceil() as usize).min(WIDTH) {
                    let distance = (px as f32 + 0.5 - x).hypot(py as f32 + 0.5 - y);
                    let alpha = (near - distance).clamp(0., 1.) * opacity;
                    rgba[(py * WIDTH + px) * 4 + 3] = (alpha * 255.).round() as u8;
                }
            }
        },
    );
    let size = [WIDTH as u32, HEIGHT as u32];
    host.set_image(SOURCE.into(), rgba.clone(), size);
    host.set_image(RELEASE_SOURCE.into(), rgba.clone(), size);
    host.set_image(PAUSED_SOURCE.into(), rgba, size);
}
