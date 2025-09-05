use sfml::{graphics::IntRect};

/// Converts a Rect with negative width or height into a normal one
pub fn normalize_rect(rect: IntRect) -> IntRect {
    let w = rect.width;
    let h = rect.height;
    let l = if w < 0 {
        rect.left + w
    } else {
        rect.left
    };
    let t = if h < 0 {
        rect.top + h
    } else {
        rect.top
    };
    IntRect::new(l, t, w.abs(), h.abs())
}

pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);

    let v = max;
    let s = if max == 0.0 { 0.0 } else { (max - min) / max };
    let h = if max == min {
        0.0
    } else {
        if r >= g && r >= b {
            60.0 * ((g - b) / (max - min) + 0.0)
        } else if g >= r && g >= b {
            60.0 * ((b - r) / (max - min) + 2.0)
        } else {
            60.0 * ((r - g) / (max - min) + 4.0)
        }
    };
    let h = (h + 360.0) % 360.0;
    (h, s, v)
}