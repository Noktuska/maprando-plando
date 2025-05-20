use hashbrown::HashMap;
use sfml::{system::Vector2f, window::{mouse::Button, Event}};

struct ClickData {
    x: f32,
    y: f32,
    frame: u32
}

pub struct MouseState {
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_dx: f32,
    pub mouse_dy: f32,

    pub buttons_pressed: Vec<Button>,
    pub buttons_released: Vec<Button>,
    pub buttons_down: Vec<Button>,

    pub button_clicked: Option<Button>,
    pub click_time_leniency: u32,
    pub click_pos_leniency: f32,

    click_data: HashMap<Button, ClickData>
}

impl Default for MouseState {
    fn default() -> Self {
        MouseState {
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            button_clicked: None,
            click_time_leniency: 60,
            click_pos_leniency: 100.0,
            buttons_pressed: Vec::new(),
            buttons_released: Vec::new(),
            buttons_down: Vec::new(),
            click_data: HashMap::new()
        }
    }
}

impl MouseState {
    pub fn next_frame(&mut self) {
        self.buttons_pressed.clear();
        self.buttons_released.clear();
        self.button_clicked = None;
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;

        for (_, data) in self.click_data.iter_mut() {
            data.frame += 1;
        }
    }

    pub fn add_event(&mut self, ev: Event) {
        match ev {
            Event::MouseButtonPressed { button, x, y } => {
                self.buttons_pressed.push(button.clone());
                self.buttons_down.push(button.clone());
                self.click_data.insert(button, ClickData { x: x as f32, y: y as f32, frame: 0 });
            }
            Event::MouseButtonReleased { button, x, y } => {
                self.buttons_released.push(button.clone());
                self.buttons_down.retain(|&x| x != button);
                if let Some(data) = self.click_data.remove(&button) {
                    let data_pos = Vector2f::new(data.x, data.y);
                    let m_pos = Vector2f::new(x as f32, y as f32);
                    let dist = (data_pos - m_pos).length_sq().sqrt();
                    if data.frame <= self.click_time_leniency && dist <= self.click_pos_leniency {
                        self.button_clicked = Some(button);
                    }
                }
            }
            Event::MouseMoved { x, y } => {
                self.mouse_dx = x as f32 - self.mouse_x;
                self.mouse_dy = y as f32 - self.mouse_y;
                self.mouse_x = x as f32;
                self.mouse_y = y as f32;
            }
            _ => {}
        }
    }

    pub fn is_button_pressed(&self, button: Button) -> bool {
        self.buttons_pressed.contains(&button)
    }

    pub fn is_button_released(&self, button: Button) -> bool {
        self.buttons_released.contains(&button)
    }

    pub fn is_button_down(&self, button: Button) -> bool {
        self.buttons_down.contains(&button)
    }

    pub fn get_mouse_pos(&self) -> Vector2f {
        Vector2f::new(self.mouse_x, self.mouse_y)
    }
}