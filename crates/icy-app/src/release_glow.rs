//! The Release notes row's hover: Discord's Quest navigation `Tab Gradient Dawn`, ported
//! literally from jsnircord (`src/row_pointer.rs`, assets/quest-hover/README.md): Rive-rendered
//! RGB basis frames recoloured at runtime, a pointer-following X on Rive's curve, and the
//! left-hand dimmer behind the label. The palette comes from the theme accent (TSX).
use jsnir::host::Host;
use resvg::tiny_skia;
use serde_json::{Value, json};
use std::{
    sync::{Mutex, Once, OnceLock},
    time::Instant,
};

/// The texture the row draws under its ink; rewritten in place frame by frame.
pub const TEXTURE: &str = "release:tab-gradient";
const TILE_W: usize = 384;
const TILE_H: usize = 44;
/// A decoded atlas in straight (non-premultiplied) RGBA.
struct Sheet {
    width: usize,
    rgba: Vec<u8>,
}

static SHEETS: OnceLock<[Sheet; 2]> = OnceLock::new();
static LOAD: Once = Once::new();

// The atlases are jsnircord's lossless WebP sheets re-encoded as PNG (pixel-identical), so the
// existing tiny-skia decoder reads them without a new image dependency.
fn sheets() -> &'static [Sheet; 2] {
    SHEETS.get_or_init(|| {
        [
            include_bytes!("../../../assets/release-hover/normal-basis.png").as_slice(),
            include_bytes!("../../../assets/release-hover/dimmed-basis.png").as_slice(),
        ]
        .map(|bytes| {
            let pixmap = tiny_skia::Pixmap::decode_png(bytes).expect("bundled checked Rive atlas");
            let width = pixmap.width() as usize;
            let mut rgba = pixmap.take();
            crate::icons::unpremultiply_rgba(&mut rgba);
            Sheet { width, rgba }
        })
    })
}

/// Tile `frame` of the sheets, `dim` (0..=255) between the normal and dimmed bakes, recoloured
/// with the centre/inner/outer `colors` (0xRRGGBB).
fn tile(frame: usize, dim: u8, colors: [u32; 3]) -> Vec<u8> {
    let [light, dark] = sheets();
    let origin = (frame / 8 * TILE_H * light.width) + (frame % 8 * TILE_W);
    let mut pixels = vec![0; TILE_W * TILE_H * 4];
    let colors = colors.map(|c| [(c >> 16) as i32, ((c >> 8) & 255) as i32, (c & 255) as i32]);
    for (y, line) in pixels
        .as_chunks_mut::<{ TILE_W * 4 }>()
        .0
        .iter_mut()
        .enumerate()
    {
        let at = (origin + y * light.width) * 4;
        let l = &light.rgba[at..at + line.len()];
        let d = &dark.rgba[at..at + line.len()];
        for ((out, l), d) in line
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(l.as_chunks::<4>().0.iter())
            .zip(d.as_chunks::<4>().0.iter())
        {
            let a = i32::from(l[3]) * (255 - i32::from(dim));
            let b = i32::from(d[3]) * i32::from(dim);
            let sum = a + b;
            if sum == 0 {
                continue;
            }
            let basis: [i32; 3] = std::array::from_fn(|k| {
                (i32::from(l[k]) * a + i32::from(d[k]) * b + sum / 2) / sum
            });
            let white = ((basis.iter().sum::<i32>() - 255) / 2).max(0);
            for k in 0..3 {
                out[k] = ((white * 255
                    + (0..3)
                        .map(|i| (basis[i] - white) * colors[i][k])
                        .sum::<i32>()
                    + 127)
                    / 255)
                    .clamp(0, 255) as u8;
            }
            out[3] = ((sum + 127) / 255) as u8;
        }
    }
    pixels
}

fn bezier(t: f32, [x1, y1, x2, y2]: [f32; 4]) -> f32 {
    if t <= 0. {
        return 0.;
    }
    if t >= 1. {
        return 1.;
    }
    let cubic = |u: f32, a: f32, b: f32| {
        3. * (1. - u) * (1. - u) * u * a + 3. * (1. - u) * u * u * b + u * u * u
    };
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..14 {
        let m = (lo + hi) * 0.5;
        if cubic(m, x1, x2) < t { lo = m } else { hi = m }
    }
    cubic((lo + hi) * 0.5, y1, y2)
}

const FOLLOW: [f32; 4] = [0.15351255, 0.28926423, 0.78115773, 0.97691095];
const EASE: [f32; 4] = [0.42, 0., 0.58, 1.];

#[derive(Clone, Copy)]
struct Segment {
    from: f32,
    to: f32,
    start: f32,
}

impl Segment {
    fn value(self, now: f32, curve: [f32; 4], duration: f32) -> f32 {
        self.from + (self.to - self.from) * bezier((now - self.start) / duration, curve)
    }
}

// Rive's interpolator advances the previous segment's value underneath
// the new one instead of starting a fresh stationary lerp on every move.
struct Follow {
    prior: Option<Segment>,
    current: Segment,
}

impl Follow {
    fn new(x: f32) -> Self {
        Self {
            prior: None,
            current: Segment {
                from: x,
                to: x,
                start: 0.,
            },
        }
    }

    fn step(&mut self, now: f32, x: f32) -> f32 {
        if let Some(p) = self.prior {
            self.current.from = p.value(now, FOLLOW, 1.);
            if now - p.start >= 1. {
                self.prior = None;
            }
        }
        let value = self.current.value(now, FOLLOW, 1.);
        if x != self.current.to {
            self.prior = (now - self.current.start < 1. && self.current.from != self.current.to)
                .then_some(self.current);
            self.current = Segment {
                from: value,
                to: x,
                start: now,
            };
        }
        value
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Watch {
    node: u16,
    /// Row-local x where the label's ink ends; left of `+32` the gradient dims (Discord's
    /// `DimmerWidth = measuredContentWidth + 32`).
    content: f32,
    colors: [u32; 3],
}

static TARGET: Mutex<Option<Watch>> = Mutex::new(None);

/// `releaseGlowTarget {id, inside, content, colors}` from the TSX row.
pub fn request(v: &Value) -> Result<String, String> {
    let node = v["id"]
        .as_u64()
        .and_then(|n| u16::try_from(n).ok())
        .ok_or("Invalid node")?;
    let mut target = TARGET.lock().unwrap();
    if v["inside"].as_bool().ok_or("Invalid state")? {
        let colors = v["colors"]
            .as_array()
            .filter(|c| c.len() == 3)
            .and_then(|c| {
                let mut out = [0u32; 3];
                for (slot, value) in out.iter_mut().zip(c) {
                    *slot = u32::try_from(value.as_u64()?).ok()? & 0xff_ffff;
                }
                Some(out)
            })
            .ok_or("Invalid palette")?;
        LOAD.call_once(|| {
            std::thread::spawn(sheets);
        });
        *target = Some(Watch {
            node,
            content: v["content"].as_f64().unwrap_or(0.) as f32,
            colors,
        });
    } else if target.is_some_and(|w| w.node == node) {
        *target = None;
    }
    Ok("ok".into())
}

/// Per-frame state on the window thread (jsnircord's `PointerTarget::sampler`).
pub struct Sampler {
    clock: Instant,
    last: Option<Watch>,
    follow: Follow,
    dim: Segment,
    texture: Option<(usize, u8, [u32; 3])>,
    geometry: Option<[f32; 4]>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            clock: Instant::now(),
            last: None,
            follow: Follow::new(154.5),
            dim: Segment {
                from: 0.,
                to: 0.,
                start: 0.,
            },
            texture: None,
            geometry: None,
        }
    }
}

impl Sampler {
    /// Advances the glow for the watched row under the pointer. `true` while it animates, so
    /// the window keeps redrawing.
    pub fn sample(&mut self, host: &mut Host, pointer: Option<[f32; 2]>) -> bool {
        let watch = *TARGET.lock().unwrap();
        if watch != self.last {
            // A new row (or palette) starts from a settled state.
            if watch.map(|w| w.node) != self.last.map(|w| w.node) {
                self.follow = Follow::new(154.5);
                self.dim = Segment {
                    from: 0.,
                    to: 0.,
                    start: 0.,
                };
                self.geometry = None;
            }
            self.texture = None;
            self.last = watch;
        }
        let Some(watch) = watch else { return false };
        let Some([left, top, w, h]) = host.screen_rect(watch.node) else {
            return false;
        };
        let Some([mx, my]) = pointer else {
            return false;
        };
        if w <= 0. || h <= 0. || mx < left || mx > left + w || my < top || my > top + h {
            return false;
        }
        let now = self.clock.elapsed().as_secs_f32();
        // `cover` of the 234×44 artboard, centred; the tile is drawn at (x − 192, 0).
        let scale = (w / 234.).max(h / 44.);
        let ax = (mx - left - w * 0.5) / scale + 117.;
        let x = self.follow.step(now, ax);
        let pos = [
            (w - 234. * scale) * 0.5 + (x - 192.) * scale,
            (h - 44. * scale) * 0.5,
            384. * scale,
            44. * scale,
        ];
        if self.geometry != Some(pos) {
            host.push(json!({"view": "releaseGlowRect", "data": pos}).to_string());
            self.geometry = Some(pos);
        }
        let dim_width = watch.content + 32.;
        let target = if ax <= dim_width { 1. } else { 0. };
        let amount = self
            .dim
            .value(now, EASE, if self.dim.to > 0. { 1.5 } else { 1. });
        if target != self.dim.to {
            self.dim = Segment {
                from: amount,
                to: target,
                start: now,
            };
        }
        // Tiles 60..179 are a complete, settled four-second pulse; JSNIR fades the row in/out.
        let key = (
            60 + ((now * 30.) as usize % 120),
            (amount * 255.).round() as u8,
            watch.colors,
        );
        if SHEETS.get().is_some() && self.texture != Some(key) {
            host.set_image(
                TEXTURE.into(),
                tile(key.0, key.1, key.2),
                [TILE_W as u32, TILE_H as u32],
            );
            self.texture = Some(key);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_recolour_the_basis_and_follow_eases_to_the_pointer() {
        let red = tile(90, 0, [0xff0000, 0xff0000, 0xff0000]);
        assert_eq!(red.len(), TILE_W * TILE_H * 4);
        let lit: Vec<_> = red.chunks(4).filter(|px| px[3] > 128).collect();
        assert!(!lit.is_empty(), "the pulse has opaque texels");
        assert!(
            lit.iter().all(|px| px[0] >= px[2]),
            "tinted towards the palette"
        );
        let mut follow = Follow::new(0.);
        follow.step(0., 60.);
        let partway = follow.step(0.5, 60.);
        assert!(partway > 20. && partway < 50., "{partway}");
        assert!((follow.step(1.0, 60.) - 60.).abs() < 0.01);
        assert!(request(&json!({"id": 3, "inside": true, "colors": [1, 2]})).is_err());
    }
}
