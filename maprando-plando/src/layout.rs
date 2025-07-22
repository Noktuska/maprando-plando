pub mod hotkey_settings;
pub mod map_editor_ui;

use egui::{Context, Ui};
use hashbrown::HashMap;
use maprando::{preset::PresetData, settings::{ETankRefill, Fanfares, ItemMarkers, MotherBrainFight, ObjectiveScreen, ObjectiveSetting, RandomizerSettings, StartingItemsPreset}};
use maprando_game::Item;

use crate::{backend::plando::Placeable, layout::hotkey_settings::HotkeySettingsWindow};

pub fn window_skill_assumptions(height: f32, open: &mut bool, cur_settings: &mut RandomizerSettings, preset_data: &PresetData, ctx: &Context) {
    egui::Window::new("Customize Skill Assumptions").collapsible(false).vscroll(true).max_height(height).resizable(false).open(open).show(ctx, |ui| {
        ui.label("Tech and notable strats");
        for diff in &preset_data.difficulty_levels.keys {
            if *diff == "Implicit" {
                continue;
            }
            let tech = &preset_data.tech_by_difficulty[diff];
            let notables = &preset_data.notables_by_difficulty[diff];
            let total = tech.len() + notables.len();
            let sel_tech_count = cur_settings.skill_assumption_settings.tech_settings.iter().filter(
                |x| x.enabled && preset_data.tech_data_map[&x.id].difficulty == *diff
            ).count();
            let sel_notable_count = cur_settings.skill_assumption_settings.notable_settings.iter().filter(
                |x| x.enabled && preset_data.notable_data_map[&(x.room_id, x.notable_id)].difficulty == *diff
            ).count();
            let sel_total = sel_tech_count + sel_notable_count;
            let percentage = (100.0 * sel_total as f32 / total as f32).round() as i32;

            let label = format!("{}, ({}%)", diff, percentage);
            ui.collapsing(label, |ui| {
                egui::Grid::new(format!("grid_tech_{diff}")).num_columns(3).show(ui, |ui| {
                    for &id in &preset_data.tech_by_difficulty[diff] {
                        let entry = cur_settings.skill_assumption_settings.tech_settings.iter_mut().find(
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
                    for &id in &preset_data.notables_by_difficulty[diff] {
                        let entry = cur_settings.skill_assumption_settings.notable_settings.iter_mut().find(
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
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.resource_multiplier).speed(0.05).max_decimals(2).range(1.0..=10.0));
            ui.end_row();

            ui.label("Escape time multiplier");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.escape_timer_multiplier).speed(0.05).max_decimals(2).range(0.0..=1000.0));
            ui.end_row();

            ui.label("Gate glitch leniency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.gate_glitch_leniency).speed(0.05).range(0..=1000));
            ui.end_row();

            ui.label("Shinecharge tiles");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.shinespark_tiles).speed(0.1).range(0.0..=1000.0));
            ui.end_row();

            ui.label("Heated Shinecharge tiles");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.heated_shinespark_tiles).speed(0.1).range(0.0..=1000.0));
            ui.end_row();

            ui.label("Speedball tiles");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.speed_ball_tiles).speed(0.1).range(0.0..=1000.0));
            ui.end_row();

            ui.label("Shinecharge leniency frames");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.shinecharge_leniency_frames).speed(0.1).range(0..=1000));
            ui.end_row();

            ui.label("Door stuck leniency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.door_stuck_leniency).speed(0.1).range(0..=1000));
            ui.end_row();

            ui.label("Bomb into Crystal Flash leniency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.bomb_into_cf_leniency).speed(0.1).range(0..=1000));
            ui.end_row();

            ui.label("Jump into Crystal Flash leniency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.jump_into_cf_leniency).speed(0.1).range(0..=1000));
            ui.end_row();

            ui.label("Spike X-Mode setup leniency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.spike_xmode_leniency).speed(0.1).range(0..=1000));
            ui.end_row();
        });

        ui.separator();
        ui.label("Boss proficiency");
        egui::Grid::new("skill_boss_proficiency").num_columns(2).show(ui, |ui| {
            ui.label("Phantoon Proficiency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.phantoon_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
            ui.end_row();

            ui.label("Draygon Proficiency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.draygon_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
            ui.end_row();

            ui.label("Ridley Proficiency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.ridley_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
            ui.end_row();

            ui.label("Botwoon Proficiency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.botwoon_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
            ui.end_row();

            ui.label("Mother Brain Proficiency");
            ui.add(egui::DragValue::new(&mut cur_settings.skill_assumption_settings.mother_brain_proficiency).speed(0.05).max_decimals(2).range(0.0..=1.0));
            ui.end_row();
        });
    });
}

pub fn window_item_progression(height: f32, open: &mut bool, cur_settings: &mut RandomizerSettings, ctx: &Context) {
    egui::Window::new("Customize Item Progression").collapsible(false).vscroll(true).max_height(height).resizable(false).open(open).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Ammo collection fraction");
            ui.add(egui::DragValue::new(&mut cur_settings.item_progression_settings.ammo_collect_fraction).speed(0.05).range(0.0..=1.0));
        });
        let pool_full = cur_settings.item_progression_settings.item_pool.iter().all(
            |x| match x.item {
                Item::Missile => x.count == 46,
                Item::ETank => x.count == 14,
                Item::ReserveTank => x.count == 4,
                Item::Super => x.count == 10,
                Item::PowerBomb => x.count == 10,
                _ => true
            }
        );
        ui.separator();
        // Item pool
        ui.horizontal(|ui| {
            ui.label("Item pool");
            if ui.selectable_label(pool_full, "Full").clicked() {
                cur_settings.item_progression_settings.item_pool.iter_mut().for_each(
                    |x| match x.item {
                        Item::Missile => x.count = 46,
                        Item::ETank => x.count = 14,
                        Item::ReserveTank => x.count = 4,
                        Item::Super => x.count = 10,
                        Item::PowerBomb => x.count = 10,
                        _ => {}
                    }
                );
            }
            if ui.selectable_label(!pool_full, "Reduced").clicked() {
                cur_settings.item_progression_settings.item_pool.iter_mut().for_each(
                    |x| match x.item {
                        Item::Missile => x.count = 12,
                        Item::ETank => x.count = 3,
                        Item::ReserveTank => x.count = 3,
                        Item::Super => x.count = 6,
                        Item::PowerBomb => x.count = 5,
                        _ => {}
                    }
                );
            }
        });
        egui::Grid::new("grid_item_pool").num_columns(2).show(ui, |ui| {
            for item in &mut cur_settings.item_progression_settings.item_pool {
                ui.label(Placeable::from_item(item.item).to_string());
                ui.add(egui::DragValue::new(&mut item.count).speed(0.1).range(0..=match item.item {
                    Item::ETank => 14,
                    Item::ReserveTank => 4,
                    _ => 100
                }));
                ui.end_row();
            }
        });
        ui.separator();

        // Starting items
        ui.horizontal(|ui| {
            let item_start_count: usize = cur_settings.item_progression_settings.starting_items.iter().map(
                |x| x.count
            ).sum();

            ui.label("Starting items");
            if ui.selectable_label(item_start_count == 0, "None").clicked() {
                cur_settings.item_progression_settings.starting_items_preset = Some(StartingItemsPreset::None);
                cur_settings.item_progression_settings.starting_items.iter_mut().for_each(
                    |x| x.count = 0
                );
            }
            if ui.selectable_label(item_start_count == 100, "All").clicked() {
                cur_settings.item_progression_settings.starting_items_preset = Some(StartingItemsPreset::All);
                cur_settings.item_progression_settings.starting_items.iter_mut().for_each(
                    |x| x.count = match x.item {
                        Item::Missile => 46,
                        Item::ETank => 14,
                        Item::ReserveTank => 4,
                        Item::Super => 10,
                        Item::PowerBomb => 10,
                        _ => 1
                    }
                );
            }
        });

        egui::Grid::new("grid_start_items").num_columns(3).show(ui, |ui| {
            for item in &mut cur_settings.item_progression_settings.starting_items {
                ui.label(Placeable::from_item(item.item).to_string());
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
}

pub fn window_qol(height: f32, open: &mut bool, cur_settings: &mut RandomizerSettings, ctx: &Context) {
    egui::Window::new("Customize Quality of life").collapsible(false).vscroll(true).max_height(height).resizable(false).open(open).show(ctx, |ui| {
        ui.label("Map");
        egui::Grid::new("grid_qol_map").num_columns(2).show(ui, |ui| {
            ui.label("Item markers");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.item_markers, ItemMarkers::Simple, "Simple");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.item_markers, ItemMarkers::Majors, "Majors");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.item_markers, ItemMarkers::Uniques, "Uniques");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.item_markers, ItemMarkers::ThreeTiered, "3-Tiered");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.item_markers, ItemMarkers::FourTiered, "4-Tiered");
            });
            ui.end_row();

            ui.label("Map stations always visible on map");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.mark_map_stations, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.mark_map_stations, true, "Yes");
            });
            ui.end_row();

            ui.label("Room outline revealed on entry");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.room_outline_revealed, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.room_outline_revealed, true, "Yes");
            });
            ui.end_row();
            
            ui.label("Opposite area connections revealed by map");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.opposite_area_revealed, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.opposite_area_revealed, true, "Yes");
            });
        });
        ui.separator();

        ui.label("End game");
        egui::Grid::new("grid_qol_endgame").num_columns(2).show(ui, |ui| {
            ui.label("Mother Brain fight (phases 2 and 3)");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.mother_brain_fight, MotherBrainFight::Vanilla, "Vanilla");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.mother_brain_fight, MotherBrainFight::Short, "Short");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.mother_brain_fight, MotherBrainFight::Skip, "Skip");
            });
            ui.end_row();

            ui.label("Supers do double damage to Mother Brain");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.supers_double, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.supers_double, true, "Yes");
            });
            ui.end_row();

            ui.label("Hyper Beam gives all movement items");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.escape_movement_items, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.escape_movement_items, true, "Yes");
            });
            ui.end_row();

            ui.label("Refill energy for escape");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.escape_refill, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.escape_refill, true, "Yes");
            });
            ui.end_row();

            ui.label("Enemies cleared during escape");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.escape_enemies_cleared, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.escape_enemies_cleared, true, "Yes");
            });
        });
        ui.separator();

        ui.label("Faster transitions");
        egui::Grid::new("grid_qol_transitions").num_columns(2).show(ui, |ui| {
            ui.label("Fast elevators");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fast_elevators, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fast_elevators, true, "Yes");
            });
            ui.end_row();

            ui.label("Fast doors");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fast_doors, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fast_doors, true, "Yes");
            });
            ui.end_row();

            ui.label("Fast pause menu");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fast_pause_menu, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fast_pause_menu, true, "Yes");
            });
            ui.end_row();

            ui.label("Item fanfares");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fanfares, Fanfares::Vanilla, "Vanilla");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fanfares, Fanfares::Trimmed, "Trimmed");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.fanfares, Fanfares::Off, "Off");
            });
        });
        ui.separator();

        ui.label("Samus control");
        egui::Grid::new("grid_qol_controls").num_columns(2).show(ui, |ui| {
            ui.label("Respin");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.respin, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.respin, true, "Yes");
            });
            ui.end_row();

            ui.label("Lenient space jump");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.infinite_space_jump, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.infinite_space_jump, true, "Yes");
            });
            ui.end_row();

            ui.label("Momentum conservation");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.momentum_conservation, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.momentum_conservation, true, "Yes");
            });
        });
        ui.separator();

        ui.label("Tweaks to unintuitive vanilla behavior");
        egui::Grid::new("grid_qol_tweaks").num_columns(2).show(ui, |ui| {
            ui.label("All items spawn at start of game");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.all_items_spawn, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.all_items_spawn, true, "Yes");
            });
            ui.end_row();

            ui.label("Acid Chozo usable without Space Jump");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.acid_chozo, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.acid_chozo, true, "Yes");
            });
            ui.end_row();

            ui.label("Lava removed from climb");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.remove_climb_lava, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.remove_climb_lava, true, "Yes");
            });
        });
        ui.separator();

        ui.label("Energy and reserves");
        egui::Grid::new("grid_qol_energy").num_columns(2).show(ui, |ui| {
            ui.label("E-Tank energy refill");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.etank_refill, ETankRefill::Disabled, "Disabled");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.etank_refill, ETankRefill::Vanilla, "Vanilla");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.etank_refill, ETankRefill::Full, "Full");
            });
            ui.end_row();

            ui.label("Energy stations refill reserves");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.energy_station_reserves, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.energy_station_reserves, true, "Yes");
            });
            ui.end_row();

            ui.label("Disableable E-Tanks");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.disableable_etanks, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.disableable_etanks, true, "Yes");
            });
            ui.end_row();

            ui.label("Reserve energy backwards transfer");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.reserve_backward_transfer, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.reserve_backward_transfer, true, "Yes");
            });
        });
        ui.separator();

        ui.label("Other");
        egui::Grid::new("grid_qol_other").num_columns(2).show(ui, |ui| {
            ui.label("Enemy drops are buffed");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.buffed_drops, false, "No");
                ui.selectable_value(&mut cur_settings.quality_of_life_settings.buffed_drops, true, "Yes");
            });
        });
    });
}

pub fn window_objectives(height: f32, open: &mut bool, cur_settings: &mut RandomizerSettings, ctx: &Context) {
    egui::Window::new("Customize Objectives").collapsible(false).vscroll(true).max_height(height).resizable(false).open(open).show(ctx, |ui| {
        let obj_groups = maprando::settings::get_objective_groups();
        let obj_map: HashMap<String, usize> = cur_settings.objective_settings.objective_options.iter().enumerate().map(
            |(i, x)| (format!("{:?}", x.objective).to_string(), i)
        ).collect();
        for group in obj_groups {
            ui.label(&group.name);
            egui::Grid::new(format!("grid_obj_{}", group.name)).num_columns(4).show(ui, |ui| {
                for (obj_internal, obj_display) in group.objectives {
                    let idx = obj_map[&obj_internal];
                    let obj = &mut cur_settings.objective_settings.objective_options[idx].setting;
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
            ui.selectable_value(&mut cur_settings.objective_settings.objective_screen, ObjectiveScreen::Disabled, "Disabled");
            ui.selectable_value(&mut cur_settings.objective_settings.objective_screen, ObjectiveScreen::Enabled, "Enabled");
        });
    });
}


pub struct Layout {
    pub hotkey_settings: HotkeySettingsWindow,

    window_stack: Vec<WindowType>
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum WindowType {
    HotkeySettings
}

impl Layout {
    pub fn new() -> Self {
        Layout {
            hotkey_settings: HotkeySettingsWindow::new(),
            window_stack: Vec::new()
        }
    }

    pub fn is_open_any(&self) -> bool {
        !self.window_stack.is_empty()
    }

    pub fn render(&mut self, ctx: &Context) {
        let mut windows_to_close = Vec::new();
        for window_type in self.window_stack.iter().cloned() {
            let window: &mut dyn LayoutWindow = match window_type {
                WindowType::HotkeySettings => &mut self.hotkey_settings,
            };

            egui::Window::new(window.get_title()).resizable(false).show(ctx, |ui| {
                if window.render(ctx, ui) {
                    windows_to_close.push(window.get_type());
                }
            });
        }

        for window_type in windows_to_close {
            self.close(window_type);
        }
    }

    pub fn open(&mut self, window_type: WindowType) {
        if self.window_stack.contains(&window_type) {
            return;
        }
        self.window_stack.push(window_type);
    }

    pub fn close(&mut self, window_type: WindowType) {
        self.window_stack.retain(|&x| x != window_type);
    }

    pub fn is_open(&self, window_type: WindowType) -> bool {
        self.window_stack.contains(&window_type)
    }
}

trait LayoutWindow {
    fn render(&mut self, ctx: &Context, ui: &mut Ui) -> bool;
    fn get_title(&self) -> String;
    fn get_type(&self) -> WindowType;
}