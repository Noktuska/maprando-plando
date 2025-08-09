use egui::{Context, Ui};
use hashbrown::HashSet;
use serde::{Deserialize, Serialize};
use sfml::window::Key;

use crate::{egui_sfml, input_state::KeyState, layout::{LayoutWindow, WindowType}};

#[derive(Serialize, Deserialize, Clone)]
pub struct Keybind {
    pub id: usize,
    name: String,
    tooltip: String,
    pub bind: HashSet<Key>,
    default: HashSet<Key>
}

impl Keybind {
    pub fn new<S: Into<String>>(id: usize, name: S, tooltip: S, binds: Vec<Key>) -> Self {
        Keybind {
            id,
            name: name.into(),
            tooltip: tooltip.into(),
            bind: HashSet::from_iter(binds.iter().cloned()),
            default: HashSet::from_iter(binds.into_iter())
        }
    }

    pub fn is_pressed(&self, key_state: &KeyState) -> bool {
        self.bind.iter().all(|key| key_state.is_key_down(key.to_owned()))
            && self.bind.iter().any(|key| key_state.is_key_pressed(key.to_owned()))
    }
}

pub struct HotkeySettingsWindow {
    keybinds: Vec<Keybind>,
    keybinds_copy: Vec<Keybind>,

    cur_rebind_idx: Option<usize>
}

impl HotkeySettingsWindow {
    pub fn new() -> Self {
        HotkeySettingsWindow {
            keybinds: Vec::new(),
            keybinds_copy: Vec::new(),
            cur_rebind_idx: None
        }
    }

    pub fn add_keybind(&mut self, bind: Keybind) {
        self.keybinds.push(bind);
    }

    pub fn get_hotkeys(&self) -> &Vec<Keybind> {
        &self.keybinds
    }
}

impl LayoutWindow for HotkeySettingsWindow {
    fn render(&mut self, _ctx: &Context, ui: &mut Ui) -> bool {
        if self.keybinds_copy.is_empty() {
            self.keybinds_copy.extend(self.keybinds.iter().cloned());
        }

        let mut close = false;
        egui::Grid::new("grid_hotkeys").num_columns(3).striped(true).show(ui, |ui| {
            for (i, bind) in self.keybinds_copy.iter_mut().enumerate() {
                ui.label(&bind.name).on_hover_text_at_pointer(&bind.tooltip);

                let bind_str: String = bind.bind.iter().map(|key| {
                    serde_json::to_string(key).unwrap().trim_matches('"').to_string()
                }).reduce(|l, r| {
                    l + " + " + &r
                }).unwrap_or(String::from("None"));

                if ui.selectable_label(self.cur_rebind_idx.is_some_and(|idx| idx == i), bind_str).clicked() {
                    self.cur_rebind_idx = Some(i);
                    bind.bind.clear();
                }

                let btn = egui::Button::new("Reset");
                if ui.add_enabled(bind.bind != bind.default, btn).clicked() {
                    bind.bind = bind.default.clone();
                }

                ui.end_row();
            }

            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    self.keybinds.clear();
                    self.keybinds.append(&mut self.keybinds_copy);
                    // Sort descending by number of keys to trigger e.g. a CTRL+C bind before a C bind
                    self.keybinds.sort_by(|l, r| r.bind.len().cmp(&l.bind.len()));
                    self.cur_rebind_idx = None;
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    self.keybinds_copy.clear();
                    self.cur_rebind_idx = None;
                    close = true;
                }
            });
        });

        if let Some(idx) = self.cur_rebind_idx {
            ui.input(|i| {
                let bind = &mut self.keybinds_copy[idx].bind;
                let mut keymap: HashSet<Key> = i.keys_down.iter().filter_map(|ekey| egui_sfml::rev_key_conv(*ekey)).collect();
                let modifiers = vec![Key::LAlt, Key::RAlt, Key::LControl, Key::RControl, Key::LShift, Key::RShift];
                for modifier in modifiers {
                    if modifier.is_pressed() {
                        keymap.insert(modifier);
                    }
                }
                if keymap.is_empty() && !bind.is_empty() { // No inputs & bind has some inputs = Input is finished
                    self.cur_rebind_idx = None;
                } else if !keymap.is_empty() { // Inputs are being held = add them to the binds
                    for key in keymap {
                        bind.insert(key);
                    }
                }
            });
        }

        close
    }

    fn get_title(&self) -> String {
        "Hotkeys".to_string()
    }

    fn get_type(&self) -> WindowType {
        WindowType::HotkeySettings
    }
}