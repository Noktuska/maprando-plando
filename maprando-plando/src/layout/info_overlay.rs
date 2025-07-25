use sfml::{graphics::{Color, Font, RectangleShape, RenderTarget, Shape, Text, Transformable}, system::Vector2f};

struct InfoOverlayLine {
    text: String,
    color: Color
}

pub struct InfoOverlayBuilder {
    lines: Vec<InfoOverlayLine>
}

impl InfoOverlayBuilder {
    pub fn new() -> Self {
        InfoOverlayBuilder {
            lines: Vec::new()
        }
    }

    pub fn new_line<S: Into<String>>(&mut self, text: S, color: Color) {
        self.lines.push(InfoOverlayLine {
            text: text.into(),
            color
        });
    }

    pub fn render(&mut self, rt: &mut dyn RenderTarget, x: f32, y: f32, max_width: f32, font: &Font, font_size: u32) {
        let x_offset = 16.0;
        let padding = 4.0;

        self.wrap_lines(max_width - x_offset - padding, font, font_size);

        let mut y_offset = 0.0;
        for line in &self.lines {
            let mut text = Text::new(&line.text, font, font_size); // TODO: Break newline when exceeding width
            text.set_fill_color(line.color);
            text.set_position(Vector2f::new(x + x_offset, y + y_offset));
            let bounds = text.global_bounds();

            let mut bg_rect = RectangleShape::new();
            bg_rect.set_position(bounds.position() + Vector2f::new(-padding, -padding));
            bg_rect.set_size(bounds.size() + Vector2f::new(padding, padding) * 2.0);
            bg_rect.set_fill_color(Color::rgba(0x1F, 0x1F, 0x1F, 0xBF));

            rt.draw(&bg_rect);
            rt.draw(&text);

            y_offset += bounds.height + padding;
        }
        self.lines.clear();
    }

    fn wrap_lines(&mut self, max_width: f32, font: &Font, font_size: u32) {
        let mut i = 0;
        while i < self.lines.len() {
            let line_str = self.lines[i].text.clone();

            let mut text = Text::new(&self.lines[i].text, font, font_size);
            let bbox = text.local_bounds();
            if bbox.width < max_width {
                i += 1;
                continue;
            }

            let space_pos: Vec<usize> = self.lines[i].text.rmatch_indices(' ').map(|x| x.0).collect();
            for pos in space_pos {
                let (l, r) = line_str.split_at(pos);

                text.set_string(l);
                if text.local_bounds().width < max_width {
                    self.lines[i].text = l.to_string();
                    self.lines.insert(i + 1, InfoOverlayLine {
                        text: r.trim().to_string(),
                        color: self.lines[i].color
                    });
                    
                    break;
                }
            }

            i += 1;
        }
    }
}