use sfml::graphics::IntRect;

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