use std::path::Path;

use anyhow::{anyhow, Result};
use egui::{Color32, Context};
use maprando::customize::{mosaic::MosaicTheme, samus_sprite::SamusSpriteCategory, ControllerButton, ControllerConfig, CustomizeSettings, DoorTheme, FlashingSetting, ItemDotChange, MusicSettings, PaletteTheme, ShakingSetting, TileTheme};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum CustomControllerButton {
    Left,
    Right,
    Up,
    Down,
    X,
    Y,
    A,
    B,
    L,
    R,
    Select,
    Start,
}

impl CustomControllerButton {
    fn convert(&self) -> ControllerButton {
        use ControllerButton::*;
        match self {
            CustomControllerButton::Left => Left,
            CustomControllerButton::Right => Right,
            CustomControllerButton::Up => Up,
            CustomControllerButton::Down => Down,
            CustomControllerButton::X => X,
            CustomControllerButton::Y => Y,
            CustomControllerButton::A => A,
            CustomControllerButton::B => B,
            CustomControllerButton::L => L,
            CustomControllerButton::R => R,
            CustomControllerButton::Select => Select,
            CustomControllerButton::Start => Start,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CustomControllerConfig {
    pub shot: CustomControllerButton,
    pub jump: CustomControllerButton,
    pub dash: CustomControllerButton,
    pub item_select: CustomControllerButton,
    pub item_cancel: CustomControllerButton,
    pub angle_up: CustomControllerButton,
    pub angle_down: CustomControllerButton,
    pub spin_lock_buttons: Vec<CustomControllerButton>,
    pub quick_reload_buttons: Vec<CustomControllerButton>,
    pub moonwalk: bool,
}

impl CustomControllerConfig {
    fn default() -> Self {
        use CustomControllerButton::*;
        CustomControllerConfig {
            shot: X,
            jump: A,
            dash: B,
            item_select: Select,
            item_cancel: Y,
            angle_up: R,
            angle_down: L,
            spin_lock_buttons: vec![X, L, R, Up],
            quick_reload_buttons: vec![L, R, Select, Start],
            moonwalk: false
        }
    }

    fn is_valid(&self) -> bool {
        let mut vec = vec![];
        vec.push(self.shot as usize);
        vec.push(self.jump as usize);
        vec.push(self.dash as usize);
        vec.push(self.item_cancel as usize);
        vec.push(self.item_select as usize);
        vec.push(self.angle_down as usize);
        vec.push(self.angle_up as usize);
        vec.sort();
        vec.dedup();
        vec.len() == 7
    }

    fn to_controller_config(&self) -> ControllerConfig {
        ControllerConfig {
            shot: self.shot.convert(),
            jump: self.jump.convert(),
            dash: self.dash.convert(),
            item_select: self.item_select.convert(),
            item_cancel: self.item_cancel.convert(),
            angle_up: self.angle_up.convert(),
            angle_down: self.angle_down.convert(),
            spin_lock_buttons: self.spin_lock_buttons.iter().map(|x| x.convert()).collect(),
            quick_reload_buttons: self.quick_reload_buttons.iter().map(|x| x.convert()).collect(),
            moonwalk: self.moonwalk
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Customization {
    pub samus_sprite: String,
    pub etank_color: [f32; 3],
    pub item_dot_change: usize,
    pub transition_letters: bool,
    pub reserve_hud_style: bool,
    pub vanilla_screw_attack_animation: bool,
    pub palette_theme: usize,
    pub tile_theme: usize,
    pub door_theme: usize,
    pub music: usize,
    pub disable_beeping: bool,
    pub shaking: usize,
    pub flashing: usize,
    pub room_names: bool,
    pub controller_config: CustomControllerConfig,
}

impl Default for Customization {
    fn default() -> Self {
        Customization {
            samus_sprite: "samus_vanilla".to_string(),
            etank_color: [0xDE as f32 / 255.0, 0x38 as f32 / 255.0, 0x94 as f32 / 255.0],
            item_dot_change: 0,
            transition_letters: true,
            reserve_hud_style: true,
            vanilla_screw_attack_animation: false,
            palette_theme: 0,
            tile_theme: 0,
            door_theme: 0,
            music: 0,
            disable_beeping: false,
            shaking: 1,
            flashing: 1,
            room_names: true,
            controller_config: CustomControllerConfig::default()
        }
    }
}

impl Customization {
    fn to_settings(&self, themes: &[MosaicTheme]) -> CustomizeSettings {
        let etank_color = Some((
            (self.etank_color[0] * 31.0) as u8,
            (self.etank_color[1] * 31.0) as u8,
            (self.etank_color[2] * 31.0) as u8
        ));

        let item_dot_change = match self.item_dot_change {
            0 => ItemDotChange::Fade,
            _ => ItemDotChange::Disappear
        };
        let palette_theme = match self.palette_theme {
            1 => PaletteTheme::AreaThemed,
            _ => PaletteTheme::Vanilla
        };
        let tile_theme = match self.tile_theme {
            0 => TileTheme::Vanilla,
            1 => TileTheme::AreaThemed,
            2 => TileTheme::Scrambled,
            i => {
                let idx = i - 2;
                if idx == themes.len() {
                    TileTheme::Constant("Outline".to_string())
                } else if idx > themes.len() {
                    TileTheme::Constant("Invisible".to_string())
                } else {
                    TileTheme::Constant(themes[idx].name.clone())
                }
            }
        };
        let door_theme = match self.door_theme {
            1 => DoorTheme::Alternate,
            _ => DoorTheme::Vanilla
        };
        let music = match self.music {
            1 => MusicSettings::Disabled,
            _ => MusicSettings::AreaThemed
        };
        let shaking = match self.shaking {
            1 => ShakingSetting::Reduced,
            2 => ShakingSetting::Disabled,
            _ => ShakingSetting::Vanilla
        };
        let flashing = match self.flashing {
            1 => FlashingSetting::Reduced,
            _ => FlashingSetting::Vanilla
        };

        CustomizeSettings {
            samus_sprite: Some(self.samus_sprite.clone()),
            etank_color,
            item_dot_change,
            transition_letters: self.transition_letters,
            reserve_hud_style: self.reserve_hud_style,
            vanilla_screw_attack_animation: self.vanilla_screw_attack_animation,
            palette_theme,
            tile_theme,
            door_theme,
            music,
            disable_beeping: self.disable_beeping,
            shaking,
            flashing,
            room_names: self.room_names,
            controller_config: self.controller_config.to_controller_config()
        }
    }
}

pub enum SettingsCustomizeResult {
    Idle, Cancel, Apply, Error(String)
}

pub struct SettingsCustomize {
    pub open: bool,

    pub customization: Customization,

    pub samus_sprite_categories: Vec<SamusSpriteCategory>,
    pub mosaic_themes: Vec<MosaicTheme>
}

impl SettingsCustomize {
    pub fn new() -> Result<Self> {
        let samus_sprites = load_samus_sprites()?;
        let mosaic_themes = vec![
            ("OuterCrateria", "Outer Crateria"),
            ("InnerCrateria", "Inner Crateria"),
            ("BlueBrinstar", "Blue Brinstar"),
            ("GreenBrinstar", "Green Brinstar"),
            ("PinkBrinstar", "Pink Brinstar"),
            ("RedBrinstar", "Red Brinstar"),
            ("UpperNorfair", "Upper Norfair"),
            ("LowerNorfair", "Lower Norfair"),
            ("WreckedShip", "Wrecked Ship"),
            ("WestMaridia", "West Maridia"),
            ("YellowMaridia", "Yellow Maridia"),
            ("MechaTourian", "Mecha Tourian"),
            ("MetroidHabitat", "Metroid Habitat"),
        ]
        .into_iter()
        .map(|(x, y)| MosaicTheme {
            name: x.to_string(),
            display_name: y.to_string(),
        })
        .collect();

        Ok(Self {
            open: false,
            customization: Customization::default(),
            samus_sprite_categories: samus_sprites,
            mosaic_themes
        })
    }

    pub fn get_settings(&self) -> CustomizeSettings {
        self.customization.to_settings(&self.mosaic_themes)
    }

    pub fn draw_customization_window(&mut self, ctx: &Context) -> SettingsCustomizeResult {
        let mut result = SettingsCustomizeResult::Idle;

        egui::Window::new("Customize")
        .resizable(false)
        .title_bar(false)
        .show(ctx, |ui| {
            let clone = self.customization.clone();

            egui::Grid::new("grid_customize").num_columns(2).striped(true).with_row_color(move |row, _| {
                let diff = Color32::from_rgb(115, 36, 36);
                let def = Customization::default();
                if vec![
                    false, false,
                    clone.door_theme != def.door_theme,
                    clone.music != def.music,
                    clone.shaking != def.shaking,
                    clone.flashing != def.flashing,
                    clone.disable_beeping != def.disable_beeping,
                    false,
                    clone.palette_theme != def.palette_theme,
                    clone.tile_theme != def.tile_theme,
                    clone.reserve_hud_style != def.reserve_hud_style,
                    clone.vanilla_screw_attack_animation != def.vanilla_screw_attack_animation,
                    clone.controller_config.shot != def.controller_config.shot,
                    clone.controller_config.jump != def.controller_config.jump,
                    clone.controller_config.dash != def.controller_config.dash,
                    clone.controller_config.item_select != def.controller_config.item_select,
                    clone.controller_config.item_cancel != def.controller_config.item_cancel,
                    clone.controller_config.angle_up != def.controller_config.angle_up,
                    clone.controller_config.angle_down != def.controller_config.angle_down,
                    clone.controller_config.quick_reload_buttons != def.controller_config.quick_reload_buttons,
                    clone.controller_config.spin_lock_buttons != def.controller_config.spin_lock_buttons,
                    clone.controller_config.moonwalk != def.controller_config.moonwalk,
                    false, false, false
                ][row] {
                    return Some(diff);
                }
                None
            }).show(ui, |ui| {
                ui.label("Samus sprite");
                egui::ComboBox::from_id_salt("combo_customize").selected_text(&self.customization.samus_sprite).show_ui(ui, |ui| {
                    for category in &self.samus_sprite_categories {
                        for sprite in &category.sprites {
                            ui.selectable_value(&mut self.customization.samus_sprite, sprite.name.clone(), sprite.display_name.clone());
                        }
                    }
                });
                ui.end_row();

                ui.label("Energy tank color");
                ui.color_edit_button_rgb(&mut self.customization.etank_color);
                ui.end_row();

                ui.label("Door colors");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.door_theme, 0, "Vanilla");
                    ui.selectable_value(&mut self.customization.door_theme, 1, "Alternate");
                });
                ui.end_row();

                ui.label("Music");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.music, 0, "On");
                    ui.selectable_value(&mut self.customization.music, 1, "Off");
                });
                ui.end_row();

                ui.label("Screen shaking");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.shaking, 0, "Vanilla");
                    ui.selectable_value(&mut self.customization.shaking, 1, "Reduced");
                    ui.selectable_value(&mut self.customization.shaking, 2, "Disabled");
                });
                ui.end_row();

                ui.label("Screen flashing");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.flashing, 0, "Vanilla");
                    ui.selectable_value(&mut self.customization.flashing, 1, "Reduced");
                });
                ui.end_row();

                ui.label("Low-energy beeping");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.disable_beeping, false, "Vanilla");
                    ui.selectable_value(&mut self.customization.disable_beeping, true, "Disabled");
                });
                ui.end_row();

                ui.separator();
                ui.end_row();

                ui.label("Room palettes");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.palette_theme, 0, "Vanilla");
                    ui.selectable_value(&mut self.customization.palette_theme, 1, "Area-themed");
                });
                ui.end_row();

                ui.label("Tile theme");
                let mut tile_theme_strs: Vec<String> = vec!["Vanilla", "Area-themed", "Scrambled"].iter().map(|x| x.to_string()).collect();
                self.mosaic_themes.iter().for_each(|x| tile_theme_strs.push(x.display_name.clone()));
                tile_theme_strs.push("Practice Outlines".to_string());
                tile_theme_strs.push("Invisible".to_string());
                egui::ComboBox::from_id_salt("combo_customize_tile").selected_text(&tile_theme_strs[self.customization.tile_theme]).show_ui(ui, |ui| {
                    for (i, theme) in tile_theme_strs.iter().enumerate() {
                        ui.selectable_value(&mut self.customization.tile_theme, i, theme);
                    }
                });
                ui.end_row();

                ui.label("Reserve tank HUD style");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.reserve_hud_style, false, "Vanilla");
                    ui.selectable_value(&mut self.customization.reserve_hud_style, true, "Revamped");
                });
                ui.end_row();

                ui.label("Screw Attack animation");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.vanilla_screw_attack_animation, true, "Vanilla");
                    ui.selectable_value(&mut self.customization.vanilla_screw_attack_animation, false, "Split");
                });
                ui.end_row();

                use CustomControllerButton::*;
                const VALUES: [CustomControllerButton; 12] = [X, Y, A, B, L, R, Select, Start, Up, Down, Left, Right];
                const STRINGS: [&str; 12] = ["X", "Y", "A", "B", "L", "R", "Select", "Start", "Up", "Down", "Left", "Right"];
                let config = &mut self.customization.controller_config;

                ui.label("Shot");
                ui.horizontal(|ui| {
                    for i in 0..7 {
                        ui.selectable_value(&mut config.shot, VALUES[i], STRINGS[i]);
                    }
                });
                ui.end_row();

                ui.label("Jump");
                ui.horizontal(|ui| {
                    for i in 0..7 {
                        ui.selectable_value(&mut config.jump, VALUES[i], STRINGS[i]);
                    }
                });
                ui.end_row();

                ui.label("Dash");
                ui.horizontal(|ui| {
                    for i in 0..7 {
                        ui.selectable_value(&mut config.dash, VALUES[i], STRINGS[i]);
                    }
                });
                ui.end_row();

                ui.label("Item Select");
                ui.horizontal(|ui| {
                    for i in 0..7 {
                        ui.selectable_value(&mut config.item_select, VALUES[i], STRINGS[i]);
                    }
                });
                ui.end_row();

                ui.label("Item Cancel");
                ui.horizontal(|ui| {
                    for i in 0..7 {
                        ui.selectable_value(&mut config.item_cancel, VALUES[i], STRINGS[i]);
                    }
                });
                ui.end_row();

                ui.label("Angle Up");
                ui.horizontal(|ui| {
                    for i in 0..7 {
                        ui.selectable_value(&mut config.angle_up, VALUES[i], STRINGS[i]);
                    }
                });
                ui.end_row();

                ui.label("Angle Down");
                ui.horizontal(|ui| {
                    for i in 0..7 {
                        ui.selectable_value(&mut config.angle_down, VALUES[i], STRINGS[i]);
                    }
                });
                ui.end_row();

                ui.label("Quick reload");
                ui.horizontal(|ui| {
                    for i in 0..VALUES.len() {
                        let resp = ui.selectable_label(config.quick_reload_buttons.contains(&VALUES[i]), STRINGS[i]);
                        if resp.clicked() {
                            if let Some(pos) = config.quick_reload_buttons.iter().position(|x| *x == VALUES[i]) {
                                config.quick_reload_buttons.remove(pos);
                            } else {
                                config.quick_reload_buttons.push(VALUES[i]);
                            }
                        }
                    }
                });
                ui.end_row();

                ui.label("Spin lock");
                ui.horizontal(|ui| {
                    for i in 0..VALUES.len() {
                        let resp = ui.selectable_label(config.spin_lock_buttons.contains(&VALUES[i]), STRINGS[i]);
                        if resp.clicked() {
                            if let Some(pos) = config.spin_lock_buttons.iter().position(|x| *x == VALUES[i]) {
                                config.spin_lock_buttons.remove(pos);
                            } else {
                                config.spin_lock_buttons.push(VALUES[i]);
                            }
                        }
                    }
                });
                ui.end_row();

                ui.label("Moonwalk");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.controller_config.moonwalk, false, "No");
                    ui.selectable_value(&mut self.customization.controller_config.moonwalk, true, "Yes");
                });
                ui.end_row();

                ui.separator();
                ui.end_row();

                while ui.button("Patch ROM").clicked() {
                    if !self.customization.controller_config.is_valid() {
                        result = SettingsCustomizeResult::Error("Controller config is invalid".to_string());
                        break;
                    }
                    result = SettingsCustomizeResult::Apply;
                }
                if ui.button("Cancel").clicked() {
                    result = SettingsCustomizeResult::Cancel;
                }
            });
        });

        result
    }
}

fn load_samus_sprites() -> Result<Vec<SamusSpriteCategory>> {
    let samus_sprites_path = Path::new("../MapRandoSprites/samus_sprites/manifest.json");
    serde_json::from_str(&std::fs::read_to_string(&samus_sprites_path)?).map_err(|err| anyhow!(err.to_string()))
}