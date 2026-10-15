//! Circular progress ring drawn with Braille characters.
//!
//! Every terminal cell holds a 2×4 grid of Braille dots, so a ring that is
//! 17 rows tall is drawn with 68 vertical dots. Terminal cells are roughly
//! twice as tall as they are wide, which makes those dots close to square –
//! so plotting in dot coordinates yields a round circle.
//!
//! The ring is *computed from the timer's progress value on every frame*
//! (there is no animation state and no separate timer), so it can never drift
//! from the countdown. Fractions run clockwise from 12 o'clock.

use super::theme::blend_linear;
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};
use std::f64::consts::TAU;

/// Dot offsets inside a cell → Braille bit.
/// Layout:  (0,0)=0x01 (1,0)=0x08 / (0,1)=0x02 (1,1)=0x10 / (0,2)=0x04 (1,2)=0x20 / (0,3)=0x40 (1,3)=0x80
const DOTS: [(usize, usize, u8); 8] = [
    (0, 0, 0x01),
    (0, 1, 0x02),
    (0, 2, 0x04),
    (1, 0, 0x08),
    (1, 1, 0x10),
    (1, 2, 0x20),
    (0, 3, 0x40),
    (1, 3, 0x80),
];

/// Ring placement in dot coordinates, relative to the widget's area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub cx: f64,
    pub cy: f64,
    pub r_out: f64,
    pub r_in: f64,
}

/// Largest centred circle that fits in `area`; stroke width scales with size.
pub fn geometry(area: Rect) -> Geometry {
    let w = area.width as f64 * 2.0;
    let h = area.height as f64 * 4.0;
    let r_out = (w.min(h) / 2.0 - 1.0).max(0.0);
    let thickness = (r_out / 7.0).clamp(2.0, 6.0).min(r_out);
    Geometry {
        cx: w / 2.0,
        cy: h / 2.0,
        r_out,
        r_in: r_out - thickness,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dot {
    /// Part of the muted background track.
    Track,
    /// Part of the progress stroke; payload is the gradient position `0..=1`.
    Active(f64),
}

/// Seamless gradient position for a clockwise fraction: 0 at 12 o'clock,
/// 1 at 6 o'clock and back to 0 – so a full ring has no hard colour seam.
fn gradient_pos(frac: f64) -> f64 {
    1.0 - (1.0 - 2.0 * frac).abs()
}

/// Classify the dot whose centre is at `(px, py)` (dot coordinates).
/// Returns `None` for dots outside the ring band.
pub fn classify(g: &Geometry, progress: f64, px: f64, py: f64) -> Option<Dot> {
    let dx = px - g.cx;
    let dy = py - g.cy;
    let d = dx.hypot(dy);
    if d > g.r_out || d < g.r_in {
        return None;
    }

    let progress = progress.clamp(0.0, 1.0);
    let mut ang = dx.atan2(-dy); // 0 at 12 o'clock, clockwise positive
    if ang < 0.0 {
        ang += TAU;
    }
    let frac = ang / TAU;

    if progress >= 1.0 {
        return Some(Dot::Active(gradient_pos(frac)));
    }
    if progress <= 0.0 {
        return Some(Dot::Track);
    }
    if frac < progress {
        return Some(Dot::Active(gradient_pos(frac)));
    }

    // Rounded caps: a small disc at the start and at the leading edge.
    let rm = (g.r_out + g.r_in) / 2.0;
    let ht = (g.r_out - g.r_in) / 2.0;

    let (sx, sy) = (g.cx, g.cy - rm);
    if (px - sx).hypot(py - sy) <= ht {
        return Some(Dot::Active(0.0));
    }

    let a = progress * TAU;
    let (ex, ey) = (g.cx + rm * a.sin(), g.cy - rm * a.cos());
    if (px - ex).hypot(py - ey) <= ht {
        return Some(Dot::Active(gradient_pos(progress)));
    }

    Some(Dot::Track)
}

/// The ring widget.
pub struct Ring {
    /// Elapsed fraction of the session, `0.0..=1.0`.
    pub progress: f64,
    /// Gradient start (12 o'clock) and far end (6 o'clock).
    pub from: Color,
    pub to: Color,
    /// Muted colour of the remaining part of the ring.
    pub track: Color,
}

impl Widget for Ring {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let g = geometry(area);
        if g.r_out < 3.0 {
            return;
        }

        let rm = (g.r_out + g.r_in) / 2.0;
        let ht = (g.r_out - g.r_in) / 2.0;
        let p_clamped = self.progress.clamp(0.0, 1.0);
        let head = if p_clamped > 0.0 && p_clamped < 1.0 {
            let a = p_clamped * TAU;
            Some((g.cx + rm * a.sin(), g.cy - rm * a.cos()))
        } else {
            None
        };

        for cy in 0..area.height {
            for cx in 0..area.width {
                let mut track_bits = 0u8;
                let mut active_bits = 0u8;
                let mut pos_sum = 0.0;
                let mut active_n = 0u32;
                let mut min_head_dist = f64::INFINITY;

                for (ox, oy, bit) in DOTS {
                    let px = (cx as usize * 2 + ox) as f64 + 0.5;
                    let py = (cy as usize * 4 + oy) as f64 + 0.5;
                    match classify(&g, self.progress, px, py) {
                        Some(Dot::Track) => track_bits |= bit,
                        Some(Dot::Active(p)) => {
                            active_bits |= bit;
                            pos_sum += p;
                            active_n += 1;
                            if let Some((hx, hy)) = head {
                                min_head_dist = min_head_dist.min((px - hx).hypot(py - hy));
                            }
                        }
                        None => {}
                    }
                }

                // A cell holds one colour. In the single cell where the stroke
                // ends we draw only the active dots, so the leading edge is
                // exact to the dot instead of snapping to the cell.
                let (bits, color) = if active_n > 0 {
                    let base = blend_linear(self.from, self.to, pos_sum / active_n as f64);
                    // Soft highlighted head at the leading edge of the arc
                    let radius = (ht * 1.6).max(2.2);
                    let color = if min_head_dist <= radius {
                        let boost = (1.0 - min_head_dist / radius) * 0.38;
                        blend_linear(base, Color::Rgb(255, 250, 244), boost)
                    } else {
                        base
                    };
                    (active_bits, color)
                } else if track_bits != 0 {
                    (track_bits, self.track)
                } else {
                    continue;
                };

                let ch = char::from_u32(0x2800 + bits as u32).unwrap_or(' ');
                let cell = buf.get_mut(area.x + cx, area.y + cy);
                cell.set_char(ch).set_fg(color);
            }
        }

        // Subtle watch-dial cardinal tick marks (12, 3, 6, 9 o'clock) just inside
        // the inner ring when the ring is large enough. Uses non-Braille '·' so
        // Braille dot accounting is untouched.
        if g.r_in >= 18.0 {
            let r_tick = g.r_in - 2.2;
            for &(tx, ty) in &[
                (g.cx, g.cy - r_tick),
                (g.cx + r_tick, g.cy),
                (g.cx, g.cy + r_tick),
                (g.cx - r_tick, g.cy),
            ] {
                let col = (tx / 2.0).floor() as u16;
                let row = (ty / 4.0).floor() as u16;
                if col < area.width && row < area.height {
                    let cell = buf.get_mut(area.x + col, area.y + row);
                    if cell.symbol() == " " {
                        cell.set_char('·').set_fg(self.track);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };

    /// Count (track, active) dots over the whole area.
    fn count(progress: f64, area: Rect) -> (usize, usize) {
        let g = geometry(area);
        let (mut track, mut active) = (0, 0);
        for y in 0..area.height as usize * 4 {
            for x in 0..area.width as usize * 2 {
                match classify(&g, progress, x as f64 + 0.5, y as f64 + 0.5) {
                    Some(Dot::Track) => track += 1,
                    Some(Dot::Active(_)) => active += 1,
                    None => {}
                }
            }
        }
        (track, active)
    }

    #[test]
    fn ring_is_a_centred_circle_in_the_smaller_dimension() {
        let g = geometry(AREA); // 80 x 80 dots
        assert_eq!(g.cx, 40.0);
        assert_eq!(g.cy, 40.0);
        assert_eq!(g.r_out, 39.0);
        assert!(g.r_in < g.r_out && g.r_in > 0.0);

        // Wide area: still limited by height, still centred.
        let wide = geometry(Rect::new(0, 0, 100, 10)); // 200 x 40 dots
        assert_eq!(wide.r_out, 19.0);
        assert_eq!(wide.cx, 100.0);
    }

    #[test]
    fn zero_progress_is_all_track() {
        let (track, active) = count(0.0, AREA);
        assert!(track > 100);
        assert_eq!(active, 0);
    }

    #[test]
    fn full_progress_is_all_active() {
        let (track, active) = count(1.0, AREA);
        assert_eq!(track, 0);
        assert!(active > 100);
    }

    #[test]
    fn active_share_matches_progress() {
        let (t0, a0) = count(0.0, AREA);
        let band = (t0 + a0) as f64;
        for p in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let (t, a) = count(p, AREA);
            assert_eq!(t + a, t0 + a0, "band never changes size");
            let share = a as f64 / band;
            // Rounded caps add a little at each end; allow a small margin.
            assert!(
                (share - p).abs() < 0.04,
                "progress {p}: active share {share:.3}"
            );
        }
    }

    #[test]
    fn active_dots_never_decrease_as_progress_grows() {
        let mut last = 0;
        for i in 0..=200 {
            let (_, a) = count(i as f64 / 200.0, AREA);
            assert!(a >= last, "ring shrank at step {i}");
            last = a;
        }
    }

    #[test]
    fn stroke_grows_clockwise_from_twelve_oclock() {
        let g = geometry(AREA);
        let rm = (g.r_out + g.r_in) / 2.0;
        // A quarter of the way round we should be at 3 o'clock.
        let at_three = classify(&g, 0.30, g.cx + rm, g.cy);
        let at_nine = classify(&g, 0.30, g.cx - rm, g.cy);
        let at_six = classify(&g, 0.30, g.cx, g.cy + rm);
        assert!(matches!(at_three, Some(Dot::Active(_))));
        assert_eq!(at_nine, Some(Dot::Track));
        assert_eq!(at_six, Some(Dot::Track));
        // Slightly past 3 o'clock is active only once progress passes 0.25.
        let just_after_three = classify(&g, 0.20, g.cx + rm, g.cy);
        assert_eq!(just_after_three, Some(Dot::Track));
    }

    #[test]
    fn gradient_position_is_seamless() {
        assert_eq!(gradient_pos(0.0), 0.0);
        assert_eq!(gradient_pos(0.5), 1.0);
        assert!((gradient_pos(1.0 - 1e-9)).abs() < 1e-6);
    }

    #[test]
    fn widget_renders_braille_and_survives_tiny_areas() {
        let mut buf = Buffer::empty(AREA);
        Ring {
            progress: 0.5,
            from: Color::Rgb(255, 0, 0),
            to: Color::Rgb(0, 0, 255),
            track: Color::Rgb(60, 60, 60),
        }
        .render(AREA, &mut buf);
        let drawn = buf
            .content()
            .iter()
            .filter(|c| ('\u{2801}'..='\u{28FF}').contains(&c.symbol().chars().next().unwrap()))
            .count();
        assert!(drawn > 50);

        for (w, h) in [(0, 0), (1, 1), (2, 1), (3, 2)] {
            let tiny = Rect::new(0, 0, w, h);
            let mut b = Buffer::empty(tiny);
            Ring {
                progress: 0.5,
                from: Color::Red,
                to: Color::Blue,
                track: Color::Gray,
            }
            .render(tiny, &mut b);
        }
    }
}
