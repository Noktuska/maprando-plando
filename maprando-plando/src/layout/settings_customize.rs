use std::path::Path;

use anyhow::{anyhow, Result};
use egui::{Color32, Context};
use hashbrown::HashSet;
use maprando::customize::{ControllerButton, ControllerConfig, CustomizeSettings, DoorTheme, FlashingSetting, ItemDotChange, MapTheme, MusicSettings, PaletteTheme, ShakingSetting, StatuesHallwayAudio, StatuesHallwayTiling, TileTheme, mosaic::MosaicTheme, samus_sprite::SamusSpriteCategory};

pub enum SettingsCustomizeResult {
    Idle, Cancel, Apply, Error(String)
}

pub struct SettingsCustomize {
    pub open: bool,

    pub customization: CustomizeSettings,

    pub samus_sprite_categories: Vec<SamusSpriteCategory>,
    pub mosaic_themes: Vec<MosaicTheme>
}

fn get_default_settings() -> CustomizeSettings {
    CustomizeSettings {
        shaking: ShakingSetting::Reduced,
        flashing: FlashingSetting::Reduced,
        tile_theme: TileTheme::Vanilla,
        vanilla_screw_attack_animation: false,
        controller_config: ControllerConfig {
            shot: ControllerButton::X,
            jump: ControllerButton::A,
            dash: ControllerButton::B,
            item_select: ControllerButton::Select,
            item_cancel: ControllerButton::Y,
            angle_up: ControllerButton::R,
            angle_down: ControllerButton::L,
            spin_lock_buttons: vec![ControllerButton::L, ControllerButton::R, ControllerButton::Up, ControllerButton::X],
            quick_reload_buttons: vec![ControllerButton::L, ControllerButton::R, ControllerButton::Select, ControllerButton::Start],
            moonwalk: true
        },
        ..Default::default()
    }
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
            ("Outline", "Practice Outlines"),
            ("Invisible", "Invisible")
        ]
        .into_iter()
        .map(|(x, y)| MosaicTheme {
            name: x.to_string(),
            display_name: y.to_string(),
        })
        .collect();

        Ok(Self {
            open: false,
            customization: CustomizeSettings::default(),
            samus_sprite_categories: samus_sprites,
            mosaic_themes
        })
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
                let def = get_default_settings();
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
                    clone.room_names != def.room_names,
                    false,
                    clone.map_theme != def.map_theme,
                    clone.item_dot_change != def.item_dot_change,
                    clone.transition_letters != def.transition_letters,
                    clone.boss_icons != def.boss_icons,
                    clone.miniboss_icons != def.miniboss_icons,
                    clone.save_icons != def.save_icons,
                    clone.statues_hallway_tiling != def.statues_hallway_tiling,
                    clone.statues_hallway_audio != def.statues_hallway_audio,
                    false,
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
                egui::ComboBox::from_id_salt("combo_customize").selected_text(self.customization.samus_sprite.as_ref().unwrap_or(&String::default())).show_ui(ui, |ui| {
                    for category in &self.samus_sprite_categories {
                        for sprite in &category.sprites {
                            ui.selectable_value(&mut self.customization.samus_sprite, Some(sprite.name.clone()), sprite.display_name.clone());
                        }
                    }
                });
                ui.end_row();

                ui.label("Energy tank color");
                let mut etank_color = if let Some((r, g, b)) = self.customization.etank_color {
                    [r as f32 / 31.0, g as f32 / 31.0, b as f32 / 31.0]
                } else {
                    [0xDE as f32 / 255.0, 0x38 as f32 / 255.0, 0x94 as f32 / 255.0]
                };
                ui.color_edit_button_rgb(&mut etank_color);
                self.customization.etank_color = Some(((etank_color[0] * 31.0) as u8, (etank_color[1] * 31.0) as u8, (etank_color[2] * 31.0) as u8));
                ui.end_row();

                ui.label("Door colors");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.door_theme, DoorTheme::Vanilla, "Vanilla");
                    ui.selectable_value(&mut self.customization.door_theme, DoorTheme::Alternate, "Alternate");
                });
                ui.end_row();

                ui.label("Music");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.music, MusicSettings::AreaThemed, "On");
                    ui.selectable_value(&mut self.customization.music, MusicSettings::Disabled, "Off");
                });
                ui.end_row();

                ui.label("Screen shaking");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.shaking, ShakingSetting::Vanilla, "Vanilla");
                    ui.selectable_value(&mut self.customization.shaking, ShakingSetting::Reduced, "Reduced");
                    ui.selectable_value(&mut self.customization.shaking, ShakingSetting::Disabled, "Disabled");
                });
                ui.end_row();

                ui.label("Screen flashing");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.flashing, FlashingSetting::Vanilla, "Vanilla");
                    ui.selectable_value(&mut self.customization.flashing, FlashingSetting::Reduced, "Reduced");
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
                    ui.selectable_value(&mut self.customization.palette_theme, PaletteTheme::Vanilla, "Vanilla");
                    ui.selectable_value(&mut self.customization.palette_theme, PaletteTheme::AreaThemed, "Area-themed");
                });
                ui.end_row();

                ui.label("Tile theme");
                let cur_tile_theme = match &self.customization.tile_theme {
                    TileTheme::Vanilla => "Vanilla".to_string(),
                    TileTheme::AreaThemed => "Area-themed".to_string(),
                    TileTheme::Scrambled => "Scrambled".to_string(),
                    TileTheme::Constant(v) => v.clone()
                };
                let mut tile_theme_strs: Vec<String> = vec!["Vanilla", "Area-themed", "Scrambled"].iter().map(|x| x.to_string()).collect();
                self.mosaic_themes.iter().for_each(|x| tile_theme_strs.push(x.display_name.clone()));
                egui::ComboBox::from_id_salt("combo_customize_tile").selected_text(&cur_tile_theme).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.customization.tile_theme, TileTheme::Vanilla, "Vanilla");
                    ui.selectable_value(&mut self.customization.tile_theme, TileTheme::AreaThemed, "Area-themed");
                    ui.selectable_value(&mut self.customization.tile_theme, TileTheme::Scrambled, "Scrambled");
                    for theme in &self.mosaic_themes {
                        ui.selectable_value(&mut self.customization.tile_theme, TileTheme::Constant(theme.name.clone()), &theme.display_name);
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

                ui.label("Room names");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.room_names, false, "Off");
                    ui.selectable_value(&mut self.customization.room_names, true, "On");
                });
                ui.end_row();
                
                ui.separator();
                ui.end_row();

                ui.label("Map theme");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.map_theme, MapTheme::Light, "Light");
                    ui.selectable_value(&mut self.customization.map_theme, MapTheme::Dark, "Dark");
                });
                ui.end_row();

                ui.label("Item dots after collection");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.item_dot_change, ItemDotChange::Stay, "Stay");
                    ui.selectable_value(&mut self.customization.item_dot_change, ItemDotChange::Fade, "Fade");
                    ui.selectable_value(&mut self.customization.item_dot_change, ItemDotChange::Disappear, "Disappear");
                });
                ui.end_row();

                ui.label("Area transition markers");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.transition_letters, false, "Arrows");
                    ui.selectable_value(&mut self.customization.transition_letters, true, "Letters");
                });
                ui.end_row();

                ui.label("Boss room icons");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.boss_icons, false, "Disabled");
                    ui.selectable_value(&mut self.customization.boss_icons, true, "Enabled");
                });
                ui.end_row();

                ui.label("Miniboss room icons");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.miniboss_icons, false, "Disabled");
                    ui.selectable_value(&mut self.customization.miniboss_icons, true, "Enabled");
                });
                ui.end_row();

                ui.label("Save room icons");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.save_icons, false, "Disabled");
                    ui.selectable_value(&mut self.customization.save_icons, true, "Enabled");
                });
                ui.end_row();

                ui.label("Statues Hallway tiling");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.statues_hallway_tiling, StatuesHallwayTiling::Disabled, "Disabled");
                    ui.selectable_value(&mut self.customization.statues_hallway_tiling, StatuesHallwayTiling::Default, "Default");
                    ui.selectable_value(&mut self.customization.statues_hallway_tiling, StatuesHallwayTiling::Enabled, "Enabled");
                });
                ui.end_row();

                ui.label("Statues Hallway audio");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.customization.statues_hallway_audio, StatuesHallwayAudio::Disabled, "Disabled");
                    ui.selectable_value(&mut self.customization.statues_hallway_audio, StatuesHallwayAudio::Enabled, "Enabled");
                    ui.selectable_value(&mut self.customization.statues_hallway_audio, StatuesHallwayAudio::Louder, "Louder");
                });
                ui.end_row();

                ui.separator();
                ui.end_row();

                use ControllerButton::*;
                const VALUES: [ControllerButton; 12] = [X, Y, A, B, L, R, Select, Start, Up, Down, Left, Right];
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
                    if !is_controls_valid(&self.customization.controller_config) {
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

fn is_controls_valid(config: &ControllerConfig) -> bool {
    let mut set = HashSet::new();
    set.insert(config.shot as usize);
    set.insert(config.jump as usize);
    set.insert(config.dash as usize);
    set.insert(config.item_cancel as usize);
    set.insert(config.item_select as usize);
    set.insert(config.angle_down as usize);
    set.insert(config.angle_up as usize);
    set.iter().count() == 7
}

fn load_samus_sprites() -> Result<Vec<SamusSpriteCategory>> {
    let samus_sprites_path = Path::new("../MapRandoSprites/samus_sprites/manifest.json");
    serde_json::from_str(&std::fs::read_to_string(&samus_sprites_path)?).map_err(|err| anyhow!(err.to_string()))
}