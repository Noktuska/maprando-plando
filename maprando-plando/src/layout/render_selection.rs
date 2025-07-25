use anyhow::Result;
use sfml::{cpp::FBox, graphics::{blend_mode::{Equation, Factor}, Color, IntRect, PrimitiveType, RectangleShape, RenderStates, RenderTarget, RenderTexture, Shape, Sprite, Texture, Transformable, Vertex}, system::{Vector2f, Vector2i}, window::ContextSettings};

pub struct RenderSelection {
    tex_border: FBox<RenderTexture>,
    tex_selection: FBox<Texture>
}

impl RenderSelection {
    pub fn new() -> Result<Self> {
        let mut tex = Texture::from_file("../plando-gfx/selection.png")?;
        tex.set_repeated(true);

        Ok(RenderSelection {
            tex_border: RenderTexture::new(1, 1)?,
            tex_selection: tex
        })
    }

    pub fn render(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates, rects: Vec<IntRect>, color: Color, sec: f32) {
        if self.tex_border.size() != rt.size() {
            if self.tex_border.recreate(rt.size().x, rt.size().y, &ContextSettings::default()).is_err() {
                return;
            }
        }

        let mut vertex_arr = Vec::with_capacity(rects.len() * 6);

        let tex_w = self.tex_selection.size().x as f32;
        let scroll = (sec * 128.0) % tex_w;
        let scroll_vec = Vector2f::new(scroll, 0.0);
        for rect in &rects {
            let tl = rect.position().as_other() * 8.0f32;
            let tr = Vector2i::new(rect.left + rect.width, rect.top).as_other() * 8.0f32;
            let bl = Vector2i::new(rect.left, rect.top + rect.height).as_other() * 8.0f32;
            let br = (rect.position() + rect.size()).as_other() * 8.0f32;
            vertex_arr.push(Vertex::new(tl, color, tl * 32.0 + scroll_vec));
            vertex_arr.push(Vertex::new(tr, color, tr * 32.0 + scroll_vec));
            vertex_arr.push(Vertex::new(bl, color, bl * 32.0 + scroll_vec));
            vertex_arr.push(Vertex::new(bl, color, bl * 32.0 + scroll_vec));
            vertex_arr.push(Vertex::new(tr, color, tr * 32.0 + scroll_vec));
            vertex_arr.push(Vertex::new(br, color, br * 32.0 + scroll_vec));
        }

        let mut states = states.clone();
        states.texture = Some(&self.tex_selection);
        //states.blend_mode = BlendMode::ADD;
        rt.draw_primitives(&vertex_arr, PrimitiveType::TRIANGLES, &states);

        self.tex_border.clear(Color::TRANSPARENT);
        states.texture = None;

        for rect in &rects {
            let mut rs = RectangleShape::with_size(rect.size().as_other() * 8.0f32 + Vector2f::new(2.0, 2.0));
            rs.set_position(rect.position().as_other() * 8.0f32 - Vector2f::new(1.0, 1.0));
            rs.set_fill_color(color);

            self.tex_border.draw_with_renderstates(&rs, &states);
        }

        states.blend_mode.alpha_src_factor = Factor::OneMinusSrcAlpha;
        states.blend_mode.alpha_dst_factor = Factor::OneMinusDstAlpha;
        states.blend_mode.alpha_equation = Equation::Subtract;
        for rect in rects {
            let mut rs = RectangleShape::with_size(rect.size().as_other() * 8.0f32);
            rs.set_position(rect.position().as_other() * 8.0f32);
            rs.set_fill_color(color);

            self.tex_border.draw_with_renderstates(&rs, &states);
        }

        self.tex_border.display();
        let spr_border = Sprite::with_texture(self.tex_border.texture());

        rt.draw(&spr_border);
    }
}