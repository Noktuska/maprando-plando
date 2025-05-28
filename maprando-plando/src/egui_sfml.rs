use anyhow::{bail, Result};
use egui::{epaint::{ClippedShape, ImageDelta, Primitive}, Context, CursorIcon, ImageData, PlatformOutput, Pos2, RawInput, TextureId, TexturesDelta, Vec2, ViewportCommand, ViewportIdMap, ViewportOutput};
use hashbrown::HashMap;
use sfml::{cpp::FBox, graphics::{blend_mode::Factor, BlendMode, Color, PrimitiveType, RenderStates, RenderTarget, RenderWindow, Texture, Vertex}, system::{Clock, Vector2, Vector2i}, window::{clipboard, mouse::Button, Cursor, CursorType, Event, Key}};

fn make_raw_input(window: &RenderWindow) -> RawInput {
    let Vector2 { x: w, y: h } = window.size();
    RawInput {
        screen_rect: Some(raw_input_screen_rect(w, h)),
        max_texture_side: Some(Texture::maximum_size() as usize),
        ..Default::default()
    }
}

fn raw_input_screen_rect(w: u32, h: u32) -> egui::Rect {
    egui::Rect {
        min: Pos2::new(0.0, 0.0),
        max: Pos2::new(w as f32, h as f32)
    }
}

fn key_conv(code: Key) -> Option<egui::Key> {
    use egui::Key as EKey;
    Some(match code {
        Key::Down => EKey::ArrowDown,
        Key::Left => EKey::ArrowLeft,
        Key::Right => EKey::ArrowRight,
        Key::Up => EKey::ArrowUp,
        Key::Escape => EKey::Escape,
        Key::Tab => EKey::Tab,
        Key::Backspace => EKey::Backspace,
        Key::Enter => EKey::Enter,
        Key::Space => EKey::Space,
        Key::Insert => EKey::Insert,
        Key::Delete => EKey::Delete,
        Key::Home => EKey::Home,
        Key::End => EKey::End,
        Key::PageUp => EKey::PageUp,
        Key::PageDown => EKey::PageDown,
        Key::LBracket => EKey::OpenBracket,
        Key::RBracket => EKey::CloseBracket,
        Key::Num0 => EKey::Num0,
        Key::Num1 => EKey::Num1,
        Key::Num2 => EKey::Num2,
        Key::Num3 => EKey::Num3,
        Key::Num4 => EKey::Num4,
        Key::Num5 => EKey::Num5,
        Key::Num6 => EKey::Num6,
        Key::Num7 => EKey::Num7,
        Key::Num8 => EKey::Num8,
        Key::Num9 => EKey::Num9,
        Key::A => EKey::A,
        Key::B => EKey::B,
        Key::C => EKey::C,
        Key::D => EKey::D,
        Key::E => EKey::E,
        Key::F => EKey::F,
        Key::G => EKey::G,
        Key::H => EKey::H,
        Key::I => EKey::I,
        Key::J => EKey::J,
        Key::K => EKey::K,
        Key::L => EKey::L,
        Key::M => EKey::M,
        Key::N => EKey::N,
        Key::O => EKey::O,
        Key::P => EKey::P,
        Key::Q => EKey::Q,
        Key::R => EKey::R,
        Key::S => EKey::S,
        Key::T => EKey::T,
        Key::U => EKey::U,
        Key::V => EKey::V,
        Key::W => EKey::W,
        Key::X => EKey::X,
        Key::Y => EKey::Y,
        Key::Z => EKey::Z,
        Key::F1 => EKey::F1,
        Key::F2 => EKey::F2,
        Key::F3 => EKey::F3,
        Key::F4 => EKey::F4,
        Key::F5 => EKey::F5,
        Key::F6 => EKey::F6,
        Key::F7 => EKey::F7,
        Key::F8 => EKey::F8,
        Key::F9 => EKey::F9,
        Key::F10 => EKey::F10,
        Key::F11 => EKey::F11,
        Key::F12 => EKey::F12,
        Key::Equal => EKey::Equals,
        Key::Hyphen => EKey::Minus,
        Key::Slash => EKey::Slash,
        Key::Tilde => EKey::Backtick,
        _ => return None,
    })
}

fn button_conv(button: Button) -> egui::PointerButton {
    match button {
        Button::Left => egui::PointerButton::Primary,
        Button::Right => egui::PointerButton::Secondary,
        Button::Middle => egui::PointerButton::Middle,
        Button::XButton1 => egui::PointerButton::Extra1,
        Button::XButton2 => egui::PointerButton::Extra2,
    }
}

fn modifier(alt: bool, ctrl: bool, shift: bool) -> egui::Modifiers {
    egui::Modifiers {
        alt,
        ctrl,
        shift,
        command: ctrl,
        mac_cmd: false
    }
}

fn get_cursor_map() -> HashMap<CursorType, FBox<Cursor>> {
    let mut map = HashMap::new();
    let values = [
        CursorType::Arrow,
        CursorType::ArrowWait,
        CursorType::Wait,
        CursorType::Text,
        CursorType::Hand,
        CursorType::SizeHorizontal,
        CursorType::SizeVertical,
        CursorType::SizeTopLeftBottomRight,
        CursorType::SizeBottomLeftTopRight,
        CursorType::SizeLeft,
        CursorType::SizeRight,
        CursorType::SizeTop,
        CursorType::SizeBottom,
        CursorType::SizeTopLeft,
        CursorType::SizeBottomRight,
        CursorType::SizeBottomLeft,
        CursorType::SizeTopRight,
        CursorType::SizeAll,
        CursorType::Cross,
        CursorType::Help,
        CursorType::NotAllowed
    ];
    for cursor_type in values {
        if let Ok(cursor) = Cursor::from_system(cursor_type) {
            map.insert(cursor_type, cursor);
        }
    }
    map
}

pub trait UserTexSource {
    fn get_texture(&mut self, id: u64) -> (f32, f32, &Texture);
}

struct DummyTexSource {
    tex: FBox<Texture>
}

impl Default for DummyTexSource {
    fn default() -> Self {
        DummyTexSource { tex: Texture::new().unwrap() }
    }
}

impl UserTexSource for DummyTexSource {
    fn get_texture(&mut self, _id: u64) -> (f32, f32, &Texture) {
        (0.0, 0.0, &self.tex)
    }
}

pub struct DrawInput {
    shapes: Vec<ClippedShape>,
    pixels_per_point: f32
}

pub struct SfEgui {
    clock: FBox<Clock>,
    ctx: Context,
    cursors: HashMap<CursorType, FBox<Cursor>>,
    last_window_pos: Vector2i,
    raw_input: RawInput,
    textures: HashMap<TextureId, FBox<Texture>>,

    pub scroll_factor: f32,
}

impl SfEgui {
    pub fn new(window: &RenderWindow) -> SfEgui {
        SfEgui {
            clock: Clock::start().unwrap(),
            ctx: Context::default(),
            cursors: get_cursor_map(),
            last_window_pos: Vector2i::default(),
            raw_input: make_raw_input(window),
            textures: HashMap::new(),

            scroll_factor: 1.0,
        }
    }

    pub fn get_context(&self) -> &Context {
        &self.ctx
    }

    pub fn add_event(&mut self, event: &Event) {
        let raw_input = &mut self.raw_input;
        match *event {
            Event::KeyPressed { code, alt, ctrl, shift, .. } => {
                if ctrl {
                    match code {
                        Key::V => raw_input.events.push(egui::Event::Paste(clipboard::get_string())),
                        Key::C => raw_input.events.push(egui::Event::Copy),
                        Key::X => raw_input.events.push(egui::Event::Cut),
                        _ => {}
                    }
                }
                if let Some(key) = key_conv(code) {
                    raw_input.events.push(egui::Event::Key {
                        key,
                        modifiers: modifier(alt, ctrl, shift),
                        pressed: true,
                        repeat: false,
                        physical_key: None
                    });
                }
            }
            Event::KeyReleased { code, alt, ctrl, shift, .. } => {
                if let Some(key) = key_conv(code) {
                    raw_input.events.push(egui::Event::Key {
                        key,
                        modifiers: modifier(alt, ctrl, shift),
                        pressed: false,
                        repeat: false,
                        physical_key: None
                    });
                }
            }
            Event::MouseMoved { x, y } => {
                raw_input.events.push(egui::Event::PointerMoved(Pos2::new(x as f32, y as f32)));
            }
            Event::MouseButtonPressed { button, x, y } => {
                let button = button_conv(button);
                let alt = Key::LAlt.is_pressed() || Key::RAlt.is_pressed();
                let shift = Key::LShift.is_pressed() || Key::RShift.is_pressed();
                let ctrl = Key::LControl.is_pressed() || Key::RControl.is_pressed();
                raw_input.events.push(egui::Event::PointerButton {
                    pos: Pos2::new(x as f32, y as f32),
                    button,
                    pressed: true,
                    modifiers: modifier(alt, ctrl, shift)
                });
            }
            Event::MouseButtonReleased { button, x, y } => {
                let button = button_conv(button);
                let alt = Key::LAlt.is_pressed() || Key::RAlt.is_pressed();
                let shift = Key::LShift.is_pressed() || Key::RShift.is_pressed();
                let ctrl = Key::LControl.is_pressed() || Key::RControl.is_pressed();
                raw_input.events.push(egui::Event::PointerButton {
                    pos: Pos2::new(x as f32, y as f32),
                    button,
                    pressed: false,
                    modifiers: modifier(alt, ctrl, shift)
                });
            }
            Event::TextEntered { unicode } => {
                if !unicode.is_control() {
                    raw_input.events.push(egui::Event::Text(unicode.to_string()));
                }
            }
            Event::MouseWheelScrolled { wheel, delta, .. } => {
                let alt = Key::LAlt.is_pressed() || Key::RAlt.is_pressed();
                let shift = Key::LShift.is_pressed() || Key::RShift.is_pressed();
                let ctrl = Key::LControl.is_pressed() || Key::RControl.is_pressed();
                if ctrl {
                    raw_input.events.push(egui::Event::Zoom(if delta > 0.0 { 1.1 } else { 0.9 }));
                } else {
                    let delta = match wheel {
                        sfml::window::mouse::Wheel::VerticalWheel => Vec2::new(0.0, delta * self.scroll_factor),
                        sfml::window::mouse::Wheel::HorizontalWheel => Vec2::new(delta * self.scroll_factor, 0.0),
                    };
                    raw_input.events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta,
                        modifiers: modifier(alt, ctrl, shift)
                    });
                }
            }
            Event::Resized { width, height } => {
                raw_input.screen_rect = Some(raw_input_screen_rect(width, height));
            }
            Event::MouseLeft => {
                raw_input.events.push(egui::Event::PointerGone);
            }
            Event::GainedFocus => {
                raw_input.events.push(egui::Event::WindowFocused(true));
            }
            Event::LostFocus => {
                raw_input.events.push(egui::Event::WindowFocused(false));
            }
            _ => {}
        }
    }

    pub fn run(&mut self, rw: &mut RenderWindow, mut f: impl FnMut(&mut RenderWindow, &Context)) -> Result<DrawInput> {
        self.prepare_raw_input();
        let out = self.ctx.run(self.raw_input.take(), |ctx| f(rw, ctx));
        self.handle_output(rw, out.platform_output, out.textures_delta, out.viewport_output)?;
        Ok(DrawInput {
            shapes: out.shapes,
            pixels_per_point: out.pixels_per_point
        })
    }

    fn prepare_raw_input(&mut self) {
        self.raw_input.time = Some(self.clock.elapsed_time().as_seconds() as f64);
        self.raw_input.modifiers.alt = Key::LAlt.is_pressed() || Key::RAlt.is_pressed();
        self.raw_input.modifiers.shift = Key::LShift.is_pressed() || Key::RShift.is_pressed();
        self.raw_input.modifiers.ctrl = Key::LControl.is_pressed() || Key::RControl.is_pressed();
    }

    fn handle_output(&mut self, rw: &mut RenderWindow, platform_output: PlatformOutput, textures_delta: TexturesDelta, viewport_output: ViewportIdMap<ViewportOutput>) -> Result<()> {
        for (id, delta) in &textures_delta.set {
            let tex = self.textures.entry(*id).or_insert_with(|| Texture::new().unwrap());
            update_tex_from_delta(tex, delta)?;
        }
        for id in &textures_delta.free {
            self.textures.remove(id);
        }
        let cursor_type = match platform_output.cursor_icon {
            CursorIcon::Default => Some(CursorType::Arrow),
            CursorIcon::None => None,
            CursorIcon::Help => Some(CursorType::Help),
            CursorIcon::Progress | CursorIcon::Wait => Some(CursorType::Wait),
            CursorIcon::Crosshair => Some(CursorType::Cross),
            CursorIcon::Text | CursorIcon::VerticalText => Some(CursorType::Text),
            CursorIcon::NotAllowed => Some(CursorType::NotAllowed),
            CursorIcon::PointingHand | CursorIcon::Grab | CursorIcon::Grabbing => Some(CursorType::Hand),
            CursorIcon::AllScroll => todo!(),
            CursorIcon::ResizeHorizontal | CursorIcon::ResizeColumn => Some(CursorType::SizeHorizontal),
            CursorIcon::ResizeNeSw => Some(CursorType::SizeBottomLeftTopRight),
            CursorIcon::ResizeNwSe => Some(CursorType::SizeTopLeftBottomRight),
            CursorIcon::ResizeVertical | CursorIcon::ResizeRow => Some(CursorType::SizeVertical),
            CursorIcon::ResizeEast => Some(CursorType::SizeRight),
            CursorIcon::ResizeSouthEast => Some(CursorType::SizeBottomRight),
            CursorIcon::ResizeSouth => Some(CursorType::SizeBottom),
            CursorIcon::ResizeSouthWest => Some(CursorType::SizeBottomLeft),
            CursorIcon::ResizeWest => Some(CursorType::SizeLeft),
            CursorIcon::ResizeNorthWest => Some(CursorType::SizeTopLeft),
            CursorIcon::ResizeNorth => Some(CursorType::SizeTop),
            CursorIcon::ResizeNorthEast => Some(CursorType::SizeTopRight),
            _ => Some(CursorType::Arrow)
        };
        let new_cursor = cursor_type.map(|t| self.cursors.get(&t).unwrap_or_else(|| self.cursors.get(&CursorType::Arrow).unwrap()));
        match new_cursor {
            Some(cur) =>  {
                rw.set_mouse_cursor_visible(true);
                unsafe {
                    rw.set_mouse_cursor(&cur);
                }
            }
            None => rw.set_mouse_cursor_visible(false),
        }

        for (_, out) in viewport_output {
            for cmd in out.commands {
                match cmd {
                    ViewportCommand::Close => rw.close(),
                    ViewportCommand::Title(s) => rw.set_title(&s),
                    ViewportCommand::Visible(visible) => {
                        if !visible {
                            self.last_window_pos = rw.position();
                        }
                        rw.set_visible(visible);
                        if visible {
                            rw.set_position(self.last_window_pos);
                        }
                    }
                    ViewportCommand::Focus => {
                        rw.request_focus();
                    }
                    _ => println!("WARN: Unhandled ViewportCommand {cmd:?}")
                }
            }
        }
        Ok(())
    }

    pub fn draw(&mut self, input: DrawInput, window: &mut RenderWindow, user_tex_src: Option<&mut dyn UserTexSource>) {
        let mut default_tex_source = DummyTexSource::default();
        let user_tex_source = user_tex_src.unwrap_or_else(|| &mut default_tex_source);
        let _ = window.set_active(true);
        unsafe {
            glu_sys::glEnable(glu_sys::GL_SCISSOR_TEST);
        }
        let mut vertices = Vec::new();
        for egui::ClippedPrimitive { clip_rect, primitive } in self.ctx.tessellate(input.shapes, input.pixels_per_point) {
            let mesh = match primitive {
                Primitive::Mesh(mesh) => mesh,
                Primitive::Callback(_callback) => continue
            };
            let (tw, th, tex) = match mesh.texture_id {
                TextureId::Managed(id) => {
                    let tex = &*self.textures[&TextureId::Managed(id)];
                    let (egui_tex_w, egui_tex_h) = (tex.size().x as f32, tex.size().y as f32);
                    (egui_tex_w, egui_tex_h, tex)
                }
                TextureId::User(id) => user_tex_source.get_texture(id)
            };
            for idx in mesh.indices {
                let v = mesh.vertices[idx as usize];
                let sf_v = Vertex::new(
                    (v.pos.x, v.pos.y).into(),
                    Color::rgba(v.color.r(), v.color.g(), v.color.b(), v.color.a()),
                    (v.uv.x * tw, v.uv.y * th).into()
                );
                vertices.push(sf_v);
            }

            let pixels_per_point = input.pixels_per_point;
            let win_size = window.size();
            let width_in_pixels = win_size.x;
            let height_in_pixels = win_size.y;

            let clip_min_x = pixels_per_point * clip_rect.min.x;
            let clip_min_y = pixels_per_point * clip_rect.min.y;
            let clip_max_x = pixels_per_point * clip_rect.max.x;
            let clip_max_y = pixels_per_point * clip_rect.max.y;

            let clip_min_x = clip_min_x.clamp(0.0, width_in_pixels as f32);
            let clip_min_y = clip_min_y.clamp(0.0, height_in_pixels as f32);
            let clip_max_x = clip_max_x.clamp(clip_min_x, width_in_pixels as f32);
            let clip_max_y = clip_max_y.clamp(clip_min_y, height_in_pixels as f32);

            let clip_min_x = clip_min_x.round() as i32;
            let clip_min_y = clip_min_y.round() as i32;
            let clip_max_x = clip_max_x.round() as i32;
            let clip_max_y = clip_max_y.round() as i32;
            unsafe {
                glu_sys::glScissor(clip_min_x, height_in_pixels as i32 - clip_max_y, clip_max_x - clip_min_x, clip_max_y - clip_min_y);
            }
            let rs = RenderStates {
                blend_mode: BlendMode {
                    color_src_factor: Factor::One,
                    color_dst_factor: Factor::OneMinusSrcAlpha,
                    alpha_src_factor: Factor::OneMinusDstAlpha,
                    alpha_dst_factor: Factor::One,
                    ..Default::default()
                },
                texture: Some(tex),
                ..Default::default()
            };
            window.draw_primitives(&vertices, PrimitiveType::TRIANGLES, &rs);
            vertices.clear();
        }
        unsafe {
            glu_sys::glDisable(glu_sys::GL_SCISSOR_TEST);
        }
        let _ = window.set_active(false);
    }
}

fn update_tex_from_delta(tex: &mut FBox<Texture>, delta: &ImageDelta) -> Result<()> {
    let [w, h] = delta.image.size();
    let [x, y] = delta.pos.map_or([0, 0], |[x, y]| [x as u32, y as u32]);
    match &delta.image {
        ImageData::Color(color) => {
            let srgba: Vec<u8> = color.pixels.iter().flat_map(|c32| c32.to_array()).collect();
            tex.update_from_pixels(&srgba, w as u32, h as u32, x, y);
        }
        ImageData::Font(font_image) => {
            let srgba: Vec<u8> = font_image.srgba_pixels(None).flat_map(|c32| c32.to_array()).collect();
            if w > tex.size().x as usize || h > tex.size().y as usize {
                if !tex.create(w as u32, h as u32).is_ok() {
                    bail!("Could not create texture of size ({w}, {h})");
                }
            }
            tex.update_from_pixels(&srgba, w as u32, h as u32, x, y);
        }
    }
    Ok(())
}