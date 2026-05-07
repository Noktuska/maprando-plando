use std::{io::Write, path::Path};

use anyhow::Result;
use egui::Context;
use hashbrown::HashMap;
use maprando::{preset::PresetData, settings::{CrashFixes, CrashFixesPreset, DisableETankSetting, DoorLocksSize, ETankRefill, EnemyDrops, EnhancedMapLevel, EnhancedMapOther, EnhancedMapSettings, EnhancedMapWalls, Fanfares, FixMode, InitialMapRevealSettings, ItemMarkers, MapRevealLevel, MapStationReveal, MotherBrainFight, ObjectiveScreen, ObjectiveSetting, RandomizerSettings, SaveAnimals, SpeedBooster, WallJump}};
use maprando_game::Item;
use maprando_plando_backend::Placeable;
use strum_macros::VariantArray;

use crate::layout::settings_gen::{SettingsGen, SettingsPreset};

enum CustomizeLogicWindow {
    None, SkillAssumption, Qol, Objectives
}

pub struct LogicCustomization {
    pub open: bool,

    pub preset_data: PresetData,
    cur_settings: RandomizerSettings, // Working state of logic settings
    cur_customize_logic_window: CustomizeLogicWindow,
    customize_window_open: bool,
    custom_preset_name: String,

    pub settings: RandomizerSettings, // Applied logic settings

    pub use_custom_escape_time: bool,
    pub custom_escape_time: usize,
    pub creator_name: String,
}

impl LogicCustomization {
    pub fn new(preset_data: PresetData, settings: RandomizerSettings) -> Self {
        Self {
            open: false,
            preset_data,
            cur_settings: settings.clone(),
            cur_customize_logic_window: CustomizeLogicWindow::None,
            customize_window_open: false,
            custom_preset_name: settings.name.as_ref().unwrap_or(&String::new()).clone(),
            settings,
            use_custom_escape_time: false,
            custom_escape_time: 0,
            creator_name: "Plando".to_string()
        }
    }

    pub fn load(&mut self, settings: RandomizerSettings, custom_escape_time: Option<usize>, creator_name: String) {
        self.settings = settings.clone();
        self.cur_settings = settings;
        self.custom_escape_time = custom_escape_time.unwrap_or(0);
        self.use_custom_escape_time = custom_escape_time.is_some();
        self.creator_name = creator_name;
    }

    pub fn draw_window(&mut self, ctx: &Context) -> Result<bool> {
        let mut should_close = Ok(false);

        egui::Window::new("Logic Customization").resizable(false).title_bar(false).show(ctx, |ui| {
            // Creator name
            ui.horizontal(|ui| {
                ui.label("Creator name").on_hover_text_at_pointer("This will replace the item progression preset. Max 9 characters");
                let text_edit = egui::TextEdit::singleline(&mut self.creator_name).char_limit(9);
                ui.add(text_edit);
            });
            // Settings preset
            ui.horizontal(|ui| {
                ui.label("Settings preset");
                let combo_text = match &self.cur_settings.name {
                    None => "Select a preset to automatically fill all settings".to_string(),
                    Some(name) => name.clone()
                };
                egui::ComboBox::from_id_salt("combo_logic_preset").selected_text(combo_text).show_ui(ui, |ui| {
                    if ui.selectable_label(self.cur_settings.name.is_none(), "Select a preset to automatically fill all settings").clicked() {
                        self.cur_settings.name = None;
                    }
                    ui.separator();
                    for preset in &self.preset_data.full_presets {
                        if ui.selectable_label(self.cur_settings.name.as_ref().is_some_and(|x| *x == *preset.name.as_ref().unwrap()), preset.name.as_ref().unwrap().clone()).clicked() {
                            self.cur_settings = preset.clone();
                            if let Some(name) = &preset.name {
                                self.custom_preset_name = name.clone();
                            }
                        }
                    }
                });
            });
            egui::Grid::new("grid_customize_logic").num_columns(9).striped(true).show(ui, |ui| {
                // Skill assumptions
                ui.label("Skill assumptions");
                for preset in &self.preset_data.skill_presets {
                    if ui.selectable_label(self.cur_settings.skill_assumption_settings == *preset, preset.preset.as_ref().unwrap()).clicked() {
                        self.cur_settings.skill_assumption_settings = preset.clone();
                    }
                }
                if ui.button("Custom").clicked() {
                    self.cur_customize_logic_window = CustomizeLogicWindow::SkillAssumption;
                }
                ui.end_row();

                // Quality of Life
                ui.label("Quality-of-life options");
                for preset in &self.preset_data.quality_of_life_presets {
                    if ui.selectable_label(self.cur_settings.quality_of_life_settings == *preset, preset.preset.as_ref().unwrap()).clicked() {
                        self.cur_settings.quality_of_life_settings = preset.clone();
                    }
                }
                if ui.button("Custom").clicked() {
                    self.cur_customize_logic_window = CustomizeLogicWindow::Qol;
                }
                ui.end_row();

                // Objectives
                ui.label("Objectives");
                for preset in &self.preset_data.objective_presets {
                    if ui.selectable_label(self.cur_settings.objective_settings == *preset, preset.preset.as_ref().unwrap()).clicked() {
                        self.cur_settings.objective_settings = preset.clone();
                    }
                }
                if ui.button("Custom").clicked() {
                    self.cur_customize_logic_window = CustomizeLogicWindow::Objectives;
                }
                ui.end_row();

                // Update objective count
                let num_obj = self.cur_settings.objective_settings.objective_options.iter().filter(
                    |x| x.setting == ObjectiveSetting::Yes
                ).count() as i32;
                self.cur_settings.objective_settings.min_objectives = num_obj;
                self.cur_settings.objective_settings.max_objectives = num_obj;
            });

            ui.horizontal(|ui| {
                ui.label("Ammo collection fraction");
                ui.add(egui::DragValue::new(&mut self.cur_settings.item_progression_settings.ammo_collect_fraction).speed(0.05).range(0.0..=1.0));
            });

            ui.collapsing("Starting Items", |ui| {
                egui::Grid::new("grid_starting_items").num_columns(4).striped(true).show(ui, |ui| {
                    for item in &mut self.cur_settings.item_progression_settings.starting_items {
                        ui.label(Placeable::from_item(item.item).map(|item| item.to_string()).unwrap_or_default());
                        if item.item.is_unique() {
                            ui.selectable_value(&mut item.count, 0, "No");
                            ui.selectable_value(&mut item.count, 1, "Yes");
                        } else {
                            ui.add(egui::DragValue::new(&mut item.count).speed(0.1).range(0..=match item.item {
                                Item::ETank => 14,
                                Item::ReserveTank => 4,
                                _ => 100
                            }));
                        }
                        ui.end_row();
                    }
                });
            });

            egui::Grid::new("grid_customize_logic_other").num_columns(4).striped(true).show(ui, |ui| {
                // Save the animals
                ui.label("Save the animals");
                ui.selectable_value(&mut self.cur_settings.save_animals, SaveAnimals::No, "No");
                ui.selectable_value(&mut self.cur_settings.save_animals, SaveAnimals::Yes, "Yes");
                ui.selectable_value(&mut self.cur_settings.save_animals, SaveAnimals::Optional, "Optional");
                ui.end_row();

                // Collectible Walljump
                ui.label("Wall Jump");
                ui.selectable_value(&mut self.cur_settings.other_settings.wall_jump, WallJump::Vanilla, "Vanilla");
                ui.selectable_value(&mut self.cur_settings.other_settings.wall_jump, WallJump::Collectible, "Collectible");
                ui.end_row();

                // Split Speed Booster
                ui.label("Speed Booster");
                ui.selectable_value(&mut self.cur_settings.other_settings.speed_booster, SpeedBooster::Vanilla, "Vanilla");
                ui.selectable_value(&mut self.cur_settings.other_settings.speed_booster, SpeedBooster::Split, "Split");
                ui.end_row();

                // Door locks size
                ui.label("Door locks size on map");
                ui.selectable_value(&mut self.cur_settings.other_settings.door_locks_size, DoorLocksSize::Small, "Small");
                ui.selectable_value(&mut self.cur_settings.other_settings.door_locks_size, DoorLocksSize::Large, "Large");
                ui.end_row();

                // Map station reveal
                ui.label("Map station activation reveal");
                ui.selectable_value(&mut self.cur_settings.other_settings.map_station_reveal, MapStationReveal::Partial, "Partial");
                ui.selectable_value(&mut self.cur_settings.other_settings.map_station_reveal, MapStationReveal::Full, "Full");
                ui.end_row();

                // Energy free shinesparks
                self.cur_settings.other_settings.energy_free_shinesparks.generate("Energy-free shinesparks", ui);

                // Ultra low qol
                // All enemies respawn
                self.cur_settings.other_settings.all_enemies_respawn.generate("All enemies respawn", ui);

                // Disable spikesuits
                self.cur_settings.other_settings.disable_spikesuit.generate("Disable spikesuits", ui);

                // Disable bluesuits
                self.cur_settings.other_settings.disable_bluesuit.generate("Disable bluesuits", ui);

                // Major glitches
                self.cur_settings.other_settings.enable_major_glitches.generate("Enable major glitches", ui);
            });
            // Save preset
            ui.horizontal(|ui| {
                ui.label("Save preset as");
                ui.text_edit_singleline(&mut self.custom_preset_name);
            });
            ui.end_row();

            // Apply / Save / Cancel
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    self.apply_presets();
                    self.settings = self.cur_settings.clone();
                    self.customize_window_open = false;
                    should_close = Ok(true);
                }
                if ui.button("Save to file").clicked() && !self.custom_preset_name.is_empty() {
                    self.apply_presets();
                    if let Err(err) = self.save_preset() {
                        should_close = Err(err);
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.cur_settings.clone_from(&self.settings);
                    self.customize_window_open = false;
                    should_close = Ok(true);
                }
            });
        });

        match self.cur_customize_logic_window {
            CustomizeLogicWindow::None => {}
            CustomizeLogicWindow::SkillAssumption => {
                self.window_skill_assumptions(ctx);
            }
            CustomizeLogicWindow::Qol => {
                self.window_qol(ctx);
            }
            CustomizeLogicWindow::Objectives => {
                self.window_objectives(ctx);
            }
        };

        if !self.customize_window_open {
            self.cur_customize_logic_window = CustomizeLogicWindow::None;
            self.customize_window_open = true;
        }

        should_close
    }

    fn window_skill_assumptions(&mut self, ctx: &Context) {
        egui::Window::new("Customize Skill Assumptions").collapsible(false).vscroll(true).resizable(false).open(&mut self.customize_window_open).show(ctx, |ui| {
            ui.label("Tech and notable strats");
            for diff in &self.preset_data.difficulty_levels.keys {
                if *diff == "Implicit" {
                    continue;
                }
                let tech = &self.preset_data.tech_by_difficulty[diff];
                let notables = &self.preset_data.notables_by_difficulty[diff];
                let total = tech.len() + notables.len();
                let sel_tech_count = self.cur_settings.skill_assumption_settings.tech_settings.iter().filter(
                    |x| x.enabled && self.preset_data.tech_data_map[&x.id].difficulty == *diff
                ).count();
                let sel_notable_count = self.cur_settings.skill_assumption_settings.notable_settings.iter().filter(
                    |x| x.enabled && self.preset_data.notable_data_map[&(x.room_id, x.notable_id)].difficulty == *diff
                ).count();
                let sel_total = sel_tech_count + sel_notable_count;
                let percentage = (100.0 * sel_total as f32 / total as f32).round() as i32;

                let label = format!("{}, ({}%)", diff, percentage);
                ui.collapsing(label, |ui| {
                    egui::Grid::new(format!("grid_tech_{diff}")).num_columns(3).show(ui, |ui| {
                        for &id in &self.preset_data.tech_by_difficulty[diff] {
                            let entry = self.cur_settings.skill_assumption_settings.tech_settings.iter_mut().find(
                                |x| x.id == id
                            );
                            if entry.is_none() {
                                continue;
                            }
                            let entry = entry.unwrap();
                            ui.label(&entry.name);
                            ui.selectable_value(&mut entry.enabled, false, "No");
                            ui.selectable_value(&mut entry.enabled, true, "Yes");
                            ui.end_row();
                        }
                    });
                    ui.separator();
                    ui.label("Notable strats");
                    egui::Grid::new(format!("grid_notable_{diff}")).num_columns(3).show(ui, |ui| {
                        for &id in &self.preset_data.notables_by_difficulty[diff] {
                            let entry = self.cur_settings.skill_assumption_settings.notable_settings.iter_mut().find(
                                |x| x.room_id == id.0 && x.notable_id == id.1
                            );
                            if entry.is_none() {
                                continue;
                            }
                            let entry = entry.unwrap();
                            ui.label(format!("{}: {}", entry.room_name, entry.notable_name));
                            ui.selectable_value(&mut entry.enabled, false, "No");
                            ui.selectable_value(&mut entry.enabled, true, "Yes");
                            ui.end_row();
                        }
                    });
                });
            }

            ui.separator();
            ui.label("Leniencies");
            egui::Grid::new("skill_general_leniency").num_columns(2).show(ui, |ui| {
                ui.label("Heat damage multiplier");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.resource_multiplier).speed(0.05).max_decimals(2).range(0.0..=10.0));
                ui.end_row();

                ui.label("Escape time multiplier");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.escape_timer_multiplier).speed(0.05).max_decimals(2).range(0.0..=1000.0));
                ui.end_row();

                ui.checkbox(&mut self.use_custom_escape_time, "Use custom escape timer (in seconds)");
                ui.add_enabled(self.use_custom_escape_time, egui::DragValue::new(&mut self.custom_escape_time).range(0..=5995));
                ui.end_row();

                ui.label("Gate glitch leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.gate_glitch_leniency).speed(0.05).range(0..=1000));
                ui.end_row();

                ui.label("Farm time limit");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.farm_time_limit).speed(0.05).range(0.0..=1000.0));
                ui.end_row();

                ui.label("Shinecharge tiles");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.shinespark_tiles).speed(0.1).range(0.0..=1000.0));
                ui.end_row();

                ui.label("Heated Shinecharge tiles");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.heated_shinespark_tiles).speed(0.1).range(0.0..=1000.0));
                ui.end_row();

                ui.label("Speedball tiles");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.speed_ball_tiles).speed(0.1).range(0.0..=1000.0));
                ui.end_row();

                ui.label("Shinecharge leniency frames");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.shinecharge_leniency_frames).speed(0.1).range(0..=1000));
                ui.end_row();

                ui.label("Door stuck leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.door_stuck_leniency).speed(0.1).range(0..=1000));
                ui.end_row();

                ui.label("Bomb into Crystal Flash leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.bomb_into_cf_leniency).speed(0.1).range(0..=1000));
                ui.end_row();

                ui.label("Jump into Crystal Flash leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.jump_into_cf_leniency).speed(0.1).range(0..=1000));
                ui.end_row();

                ui.label("Spikesuit leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.spike_suit_leniency).speed(0.1).range(0..=1000));
                ui.end_row();

                ui.label("Flash suit carrying distance");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.flash_suit_distance).speed(0.1).range(0..=255));
                ui.end_row();
                
                ui.label("Blue suit carrying distance");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.blue_suit_distance).speed(0.1).range(0..=255));
                ui.end_row();

                ui.label("Spike X-Mode setup leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.spike_xmode_leniency).speed(0.1).range(0..=1000));
                ui.end_row();

                ui.label("Spike speed keep leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.spike_speed_keep_leniency).speed(0.1).range(0..=1000));
                ui.end_row();

                ui.label("Elevator Crystal Flash leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.elevator_cf_leniency).speed(0.1).range(0..=1000));
                ui.end_row();
                
                ui.label("Crystal Spark leniency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.crystal_spark_leniency).speed(0.1).range(0..=1000));
                ui.end_row();
            });

            ui.separator();
            ui.label("Boss proficiency");
            egui::Grid::new("skill_boss_proficiency").num_columns(2).show(ui, |ui| {
                ui.label("Phantoon Proficiency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.phantoon_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
                ui.end_row();

                ui.label("Draygon Proficiency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.draygon_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
                ui.end_row();

                ui.label("Ridley Proficiency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.ridley_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
                ui.end_row();

                ui.label("Botwoon Proficiency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.botwoon_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
                ui.end_row();

                ui.label("Mother Brain Proficiency");
                ui.add(egui::DragValue::new(&mut self.cur_settings.skill_assumption_settings.mother_brain_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
                ui.end_row();
            });
        });
    }

    fn window_qol(&mut self, ctx: &Context) {
        egui::Window::new("Customize Quality of life").collapsible(false).vscroll(true).resizable(false).open(&mut self.customize_window_open).show(ctx, |ui| {
            let qol = &mut self.cur_settings.quality_of_life_settings;
            
            ui.label("Map");
            egui::Grid::new("grid_qol_map").num_columns(5).show(ui, |ui| {
                let enhanced_map_settings = &mut qol.enhanced_map_settings;
                enhanced_map_settings.generate("Enhanced map", ui);
                ui.end_row();

                ui.collapsing("Custom", |ui| {
                    egui::Grid::new("grid_qol_enhanced_map").num_columns(3).show(ui, |ui| {
                        enhanced_map_settings.blue_doors.generate("Blue doors", ui);
                        enhanced_map_settings.gray_doors.generate("Blue doors", ui);
                        enhanced_map_settings.ammo_doors.generate("Blue doors", ui);
                        enhanced_map_settings.beam_doors.generate("Blue doors", ui);
                        enhanced_map_settings.heat.generate("Blue doors", ui);
                        enhanced_map_settings.water.generate("Blue doors", ui);
                        enhanced_map_settings.lava.generate("Blue doors", ui);
                        enhanced_map_settings.acid.generate("Blue doors", ui);
                        enhanced_map_settings.walls.generate("Blue doors", ui);
                        enhanced_map_settings.objectives.generate("Blue doors", ui);
                        enhanced_map_settings.map_station.generate("Blue doors", ui);
                        enhanced_map_settings.refill_station.generate("Blue doors", ui);
                    })
                });

                let s = &mut qol.initial_map_reveal_settings;
                s.generate("Initial Map Reveal", ui);
                ui.end_row();

                ui.collapsing("Custom", |ui| {
                    egui::Grid::new("grid_qol_map_reveal").num_columns(4).show(ui, |ui| {
                        s.map_stations.generate("Map stations", ui);
                        s.save_stations.generate("Save stations", ui);
                        s.refill_stations.generate("Refill stations", ui);
                        s.ship.generate("Ship", ui);
                        s.objectives.generate("Objectives", ui);
                        s.area_transitions.generate("Area transitions", ui);
                        s.items1.generate("Items: tier 1 (small dots)", ui);
                        s.items2.generate("Items: tier 2 (X's)", ui);
                        s.items3.generate("Items: tier 3 (hollow circles)", ui);
                        s.items4.generate("Items: tier 4 (large dots)", ui);
                        s.other.generate("Other", ui);
                        s.all_areas.generate("Reveal tiles in unvisited areas", ui);
                    });
                });
                ui.end_row();

                ui.label("Item markers");
                ui.selectable_value(&mut qol.item_markers, ItemMarkers::Simple, "Simple");
                ui.selectable_value(&mut qol.item_markers, ItemMarkers::Majors, "Majors");
                ui.selectable_value(&mut qol.item_markers, ItemMarkers::Uniques, "Uniques");
                ui.selectable_value(&mut qol.item_markers, ItemMarkers::ThreeTiered, "3-Tiered");
                ui.selectable_value(&mut qol.item_markers, ItemMarkers::FourTiered, "4-Tiered");
                ui.end_row();

                qol.room_outline_revealed.generate("Room outline revealed on entry", ui);
                qol.opposite_area_revealed.generate("Opposite area connections revealed by map", ui);
                qol.hazard_markers.generate("Hazard markers", ui);
            });
            ui.separator();

            ui.label("End game");
            egui::Grid::new("grid_qol_endgame").num_columns(2).show(ui, |ui| {
                ui.label("Mother Brain fight (phases 2 and 3)");
                ui.selectable_value(&mut qol.mother_brain_fight, MotherBrainFight::Vanilla, "Vanilla");
                ui.selectable_value(&mut qol.mother_brain_fight, MotherBrainFight::Short, "Short");
                ui.selectable_value(&mut qol.mother_brain_fight, MotherBrainFight::Skip, "Skip");
                ui.end_row();

                qol.supers_double.generate("Supers do double damage to Mother Brain", ui);
                qol.escape_autosave.generate("Escape auto save", ui);
                qol.escape_movement_items.generate("Hyper Beam gives all movement items", ui);
                qol.escape_refill.generate("Refill energy for escape", ui);
                qol.escape_enemies_cleared.generate("Enemies cleared during escape", ui);
            });
            ui.separator();

            ui.label("Faster transitions");
            egui::Grid::new("grid_qol_transitions").num_columns(2).show(ui, |ui| {
                qol.fast_elevators.generate("Fast elevators", ui);
                qol.fast_doors.generate("Fast doors", ui);
                qol.fast_pause_menu.generate("Fast pause menu", ui);
                qol.fast_saves.generate("Fast saves", ui);
                qol.fast_baby_cutscene.generate("Fast baby cutscene", ui);
                qol.fast_mother_brain_cutscene.generate("Fast Mother Brain cutscene", ui);
                qol.fast_decompression.generate("Fast decompression", ui);

                ui.label("Item fanfares");
                ui.selectable_value(&mut qol.fanfares, Fanfares::Vanilla, "Vanilla");
                ui.selectable_value(&mut qol.fanfares, Fanfares::Trimmed, "Trimmed");
                ui.selectable_value(&mut qol.fanfares, Fanfares::Off, "Off");
            });
            ui.separator();

            ui.label("Samus control");
            egui::Grid::new("grid_qol_controls").num_columns(2).show(ui, |ui| {
                qol.respin.generate("Respin", ui);
                qol.infinite_space_jump.generate("Lenient space jump", ui);
                qol.momentum_conservation.generate("Momentum conservation", ui);
            });
            ui.separator();

            ui.label("Tweaks to unintuitive vanilla behavior");
            egui::Grid::new("grid_qol_tweaks").num_columns(2).show(ui, |ui| {
                qol.all_items_spawn.generate("All items spawn at start of game", ui);
                qol.acid_chozo.generate("Acid Chozo usable without Space Jump", ui);
                qol.remove_climb_lava.generate("Lava removed from climb", ui);
                qol.crash_fixes.generate("Crash fixes", ui);

                ui.collapsing("Custom", |ui| {
                    egui::Grid::new("grid_qol_crash_fixes").num_columns(5).show(ui, |ui| {
                        qol.crash_fixes.spring_ball.generate("Unequip Spring Ball bounce", ui);
                        qol.crash_fixes.yapping_maw.generate("Yapping Maw shinespark", ui);
                        qol.crash_fixes.auto_reserve.generate("Frame-perfect pause auto-reserve", ui);
                        qol.crash_fixes.x_mode.generate("X-Mode tile collision", ui);
                        qol.crash_fixes.sprite_overflow.generate("Sprite overflow bug", ui);
                    })
                });

                qol.fix_blue_echoes.generate("Fix blue speed echoes", ui);
            });
            ui.separator();

            ui.label("Ammo, energy and reserves");
            egui::Grid::new("grid_qol_energy").num_columns(2).show(ui, |ui| {
                qol.ammo_refill_all.generate("Ammo stations refill all ammo types", ui);
                qol.energy_station_reserves.generate("Energy stations refill reserves", ui);

                ui.label("E-Tank energy refill");
                ui.selectable_value(&mut qol.etank_refill, ETankRefill::Disabled, "Disabled");
                ui.selectable_value(&mut qol.etank_refill, ETankRefill::Vanilla, "Vanilla");
                ui.selectable_value(&mut qol.etank_refill, ETankRefill::Full, "Full");
                ui.end_row();

                ui.label("Disableable E-Tanks");
                ui.selectable_value(&mut qol.disableable_etanks, DisableETankSetting::Off, "Off");
                ui.selectable_value(&mut qol.disableable_etanks, DisableETankSetting::Standard, "Standard");
                ui.selectable_value(&mut qol.disableable_etanks, DisableETankSetting::Unrestricted, "Unrestricted");
                ui.end_row();

                qol.reserve_backward_transfer.generate("Reserve energy backwards transfer", ui);
            });
            ui.separator();

            ui.label("Other");
            egui::Grid::new("grid_qol_other").num_columns(2).show(ui, |ui| {
                ui.label("Enemy drops");
                ui.selectable_value(&mut qol.enemy_drops, EnemyDrops::Off, "Off");
                ui.selectable_value(&mut qol.enemy_drops, EnemyDrops::Vanilla, "Vanilla");
                ui.selectable_value(&mut qol.enemy_drops, EnemyDrops::Buffed, "Buffed");
                ui.end_row();
                
                qol.early_save.generate("Guaranteed early save station", ui);
                qol.persist_flash_suit.generate("Flash suit persists across save/load", ui);
                qol.persist_blue_suit.generate("Blue suit persists across save/load", ui);
                qol.camera_fixes.generate("Camera fixes", ui);
            });
        });
    }

    fn window_objectives(&mut self, ctx: &Context) {
        egui::Window::new("Customize Objectives").collapsible(false).vscroll(true).resizable(false).open(&mut self.customize_window_open).show(ctx, |ui| {
        let obj_groups = maprando::settings::get_objective_groups();
        let obj_map: HashMap<String, usize> = self.cur_settings.objective_settings.objective_options.iter().enumerate().map(
            |(i, x)| (format!("{:?}", x.objective).to_string(), i)
        ).collect();
        for group in obj_groups {
            ui.label(&group.name);
            egui::Grid::new(format!("grid_obj_{}", group.name)).num_columns(4).show(ui, |ui| {
                for (obj_internal, obj_display) in group.objectives {
                    let idx = obj_map[&obj_internal];
                    let obj = &mut self.cur_settings.objective_settings.objective_options[idx].setting;
                    ui.label(obj_display);
                    ui.selectable_value(obj, ObjectiveSetting::No, "No");
                    ui.selectable_value(obj, ObjectiveSetting::Yes, "Yes");
                    ui.end_row();
                }
            });
            ui.separator();
        }

        ui.horizontal(|ui| {
            ui.label("Pause menu objective screen");
            ui.selectable_value(&mut self.cur_settings.objective_settings.objective_screen, ObjectiveScreen::Disabled, "Disabled");
            ui.selectable_value(&mut self.cur_settings.objective_settings.objective_screen, ObjectiveScreen::Enabled, "Enabled");
        });
    });
    }

    pub const CUSTOM_PRESETS_PATH: &'static str = "../../custom-presets/";

    fn save_preset(&mut self) -> Result<()> {
        self.cur_settings.name = Some(self.custom_preset_name.clone());

        let str = serde_json::to_string_pretty(&self.cur_settings)?;
        let dir = Path::new(Self::CUSTOM_PRESETS_PATH);
        let path = dir.join(format!("{}.json", self.custom_preset_name));
        let mut file = std::fs::File::create(path)?;
        file.write_all(str.as_bytes())?;

        self.preset_data.full_presets.push(self.cur_settings.clone());

        Ok(())
    }

    fn apply_presets(&mut self) {
        match self.preset_data.skill_presets.iter().find(|x| **x == self.cur_settings.skill_assumption_settings) {
            Some(preset) => self.cur_settings.skill_assumption_settings.preset = preset.preset.clone(),
            None => self.cur_settings.skill_assumption_settings.preset = None
        }
        self.cur_settings.item_progression_settings.preset = None;
        match self.preset_data.quality_of_life_presets.iter().find(|x| **x == self.cur_settings.quality_of_life_settings) {
            Some(preset) => self.cur_settings.quality_of_life_settings.preset = preset.preset.clone(),
            None => self.cur_settings.quality_of_life_settings.preset = None
        }
        match self.preset_data.objective_presets.iter().find(|x| **x == self.cur_settings.objective_settings) {
            Some(preset) => self.cur_settings.objective_settings.preset = preset.preset.clone(),
            None => self.cur_settings.objective_settings.preset = None
        }
        self.cur_settings.name = None;
    }
}

impl SettingsGen for MapRevealLevel {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut egui::Ui) {
        ui.label(label.into());
        ui.selectable_value(self, MapRevealLevel::No, "No");
        ui.selectable_value(self, MapRevealLevel::Partial, "Partial");
        ui.selectable_value(self, MapRevealLevel::Full, "Full");
        ui.end_row();
    }
}

#[derive(VariantArray, PartialEq)]
enum MapRevealPreset {
    No, Maps, Partial, Full, Global
}

impl ToString for MapRevealPreset {
    fn to_string(&self) -> String {
        match self {
            MapRevealPreset::No => "No",
            MapRevealPreset::Maps => "Maps",
            MapRevealPreset::Partial => "Partial",
            MapRevealPreset::Full => "Full",
            MapRevealPreset::Global => "Global",
        }.to_string()
    }
}

impl SettingsPreset<MapRevealPreset> for InitialMapRevealSettings {
    fn get(key: &MapRevealPreset) -> Self {
        match key {
            MapRevealPreset::No => InitialMapRevealSettings {
                all_areas: false,
                preset: Some("No".to_string()),
                map_stations: MapRevealLevel::No,
                save_stations: MapRevealLevel::No,
                refill_stations: MapRevealLevel::No,
                ship: MapRevealLevel::No,
                objectives: MapRevealLevel::No,
                area_transitions: MapRevealLevel::No,
                items1: MapRevealLevel::No,
                items2: MapRevealLevel::No,
                items3: MapRevealLevel::No,
                items4: MapRevealLevel::No,
                other: MapRevealLevel::No,
            },
            MapRevealPreset::Maps => InitialMapRevealSettings {
                all_areas: false,
                preset: Some("Maps".to_string()),
                map_stations: MapRevealLevel::Full,
                save_stations: MapRevealLevel::No,
                refill_stations: MapRevealLevel::No,
                ship: MapRevealLevel::No,
                objectives: MapRevealLevel::No,
                area_transitions: MapRevealLevel::No,
                items1: MapRevealLevel::No,
                items2: MapRevealLevel::No,
                items3: MapRevealLevel::No,
                items4: MapRevealLevel::No,
                other: MapRevealLevel::No,
            },
            MapRevealPreset::Partial => InitialMapRevealSettings {
                all_areas: false,
                preset: Some("Partial".to_string()),
                map_stations: MapRevealLevel::Partial,
                save_stations: MapRevealLevel::Partial,
                refill_stations: MapRevealLevel::Partial,
                ship: MapRevealLevel::Partial,
                objectives: MapRevealLevel::Partial,
                area_transitions: MapRevealLevel::Partial,
                items1: MapRevealLevel::Partial,
                items2: MapRevealLevel::Partial,
                items3: MapRevealLevel::Partial,
                items4: MapRevealLevel::Partial,
                other: MapRevealLevel::Partial,
            },
            MapRevealPreset::Full => InitialMapRevealSettings {
                all_areas: false,
                preset: Some("Full".to_string()),
                map_stations: MapRevealLevel::Full,
                save_stations: MapRevealLevel::Full,
                refill_stations: MapRevealLevel::Full,
                ship: MapRevealLevel::Full,
                objectives: MapRevealLevel::Full,
                area_transitions: MapRevealLevel::Full,
                items1: MapRevealLevel::Full,
                items2: MapRevealLevel::Full,
                items3: MapRevealLevel::Full,
                items4: MapRevealLevel::Full,
                other: MapRevealLevel::Full,
            },
            MapRevealPreset::Global => InitialMapRevealSettings {
                all_areas: true,
                preset: Some("Global".to_string()),
                map_stations: MapRevealLevel::Full,
                save_stations: MapRevealLevel::Full,
                refill_stations: MapRevealLevel::Full,
                ship: MapRevealLevel::Full,
                objectives: MapRevealLevel::Full,
                area_transitions: MapRevealLevel::Full,
                items1: MapRevealLevel::Full,
                items2: MapRevealLevel::Full,
                items3: MapRevealLevel::Full,
                items4: MapRevealLevel::Full,
                other: MapRevealLevel::Full,
            },
        }
    }
}

// GENERATOR FOR EnhancedMapSettings
#[derive(VariantArray, PartialEq)]
enum EnhancedMapPreset {
    No, Yes
}

impl ToString for EnhancedMapPreset {
    fn to_string(&self) -> String {
        match *self {
            Self::No => "No",
            Self::Yes => "Yes"
        }.to_string()
    }
}

impl SettingsPreset<EnhancedMapPreset> for EnhancedMapSettings {
    fn get(key: &EnhancedMapPreset) -> Self {
        match *key {
            EnhancedMapPreset::No => EnhancedMapSettings {
                preset: Some("No".to_string()),
                blue_doors: EnhancedMapLevel::Hidden,
                gray_doors: EnhancedMapLevel::Hidden,
                ammo_doors: EnhancedMapLevel::Hidden,
                beam_doors: EnhancedMapLevel::Hidden,
                heat: EnhancedMapLevel::Hidden,
                water: EnhancedMapLevel::Hidden,
                lava: EnhancedMapLevel::Hidden,
                acid: EnhancedMapLevel::Hidden,
                walls: EnhancedMapWalls::Vanilla,
                objectives: EnhancedMapOther::Vanilla,
                map_station: EnhancedMapOther::Vanilla,
                refill_station: EnhancedMapOther::Vanilla
            },
            EnhancedMapPreset::Yes => EnhancedMapSettings {
                preset: Some("Yes".to_string()),
                blue_doors: EnhancedMapLevel::Visible,
                gray_doors: EnhancedMapLevel::Visible,
                ammo_doors: EnhancedMapLevel::Visible,
                beam_doors: EnhancedMapLevel::Visible,
                heat: EnhancedMapLevel::Visible,
                water: EnhancedMapLevel::Visible,
                lava: EnhancedMapLevel::Visible,
                acid: EnhancedMapLevel::Visible,
                walls: EnhancedMapWalls::Enhanced,
                objectives: EnhancedMapOther::Icon,
                map_station: EnhancedMapOther::Icon,
                refill_station: EnhancedMapOther::Icon
            }
        }
    }
}

impl SettingsGen for EnhancedMapLevel {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut egui::Ui) {
        ui.label(label.into());
        ui.selectable_value(self, Self::Hidden, "Hidden");
        ui.selectable_value(self, Self::Visible, "Visible");
        ui.end_row();
    }
}

impl SettingsGen for EnhancedMapOther {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut egui::Ui) {
        ui.label(label.into());
        ui.selectable_value(self, Self::Vanilla, "Vanilla");
        ui.selectable_value(self, Self::Icon, "Icon");
        ui.end_row();
    }
}

impl SettingsGen for EnhancedMapWalls {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut egui::Ui) {
        ui.label(label.into());
        ui.selectable_value(self, Self::Vanilla, "Vanilla");
        ui.selectable_value(self, Self::Enhanced, "Enhanced");
        ui.end_row();
    }
}

// GENERATOR FOR CrashFixes

#[derive(VariantArray, PartialEq)]
enum CrashFixPreset {
    Crash, Death, Warn, Silent
}

impl ToString for CrashFixPreset {
    fn to_string(&self) -> String {
        match *self {
            Self::Crash => "Crash",
            Self::Death => "Death",
            Self::Warn => "Warn",
            Self::Silent => "Silent"
        }.to_string()
    }
}

impl SettingsPreset<CrashFixPreset> for CrashFixes {
    fn get(key: &CrashFixPreset) -> Self {
        match *key {
            CrashFixPreset::Crash => Self::from_preset(CrashFixesPreset::Crash),
            CrashFixPreset::Death => Self::from_preset(CrashFixesPreset::Death),
            CrashFixPreset::Warn => Self::from_preset(CrashFixesPreset::Warn),
            CrashFixPreset::Silent => Self::from_preset(CrashFixesPreset::Silent),
        }
    }
}

impl SettingsGen for FixMode {
    fn generate<S: Into<String>>(&mut self, label: S, ui: &mut egui::Ui) {
        ui.label(label.into());
        ui.selectable_value(self, Self::Crash, "Crash");
        ui.selectable_value(self, Self::Death, "Death");
        ui.selectable_value(self, Self::Warn, "Warn");
        ui.selectable_value(self, Self::Silent, "Silent");
        ui.end_row();
    }
}