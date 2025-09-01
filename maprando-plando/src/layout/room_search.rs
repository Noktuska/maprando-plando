use egui::Ui;
use maprando_game::{GameData, Map};

#[derive(PartialEq)]
pub enum SearchOpt {
    Any, Yes, No
}

impl SearchOpt {
    fn compare(&self, v: bool) -> bool {
        *self == SearchOpt::Any || (*self == SearchOpt::Yes) == v
    }
}

impl ToString for SearchOpt {
    fn to_string(&self) -> String {
        match self {
            SearchOpt::Any => "Any",
            SearchOpt::Yes => "Yes",
            SearchOpt::No => "No"
        }.to_string()
    }
}

pub struct RoomSearch {
    pub name: String,
    pub is_heated: SearchOpt,
    pub min_width: usize,
    pub max_width: usize,
    pub min_height: usize,
    pub max_height: usize,
    pub min_door_count: [usize; 4], // Right, Down, Left, Up
    pub max_door_count: [usize; 4],
    pub min_items: usize,
    pub max_items: usize,
    pub is_placed: SearchOpt,
}

impl Default for RoomSearch {
    fn default() -> Self {
        Self {
            name: "".to_string(),
            is_heated: SearchOpt::Any,
            min_width: 0,
            max_width: 99,
            min_height: 0,
            max_height: 99,
            min_door_count: [0; 4],
            max_door_count: [9; 4],
            min_items: 0,
            max_items: 3,
            is_placed: SearchOpt::Any,
        }
    }
}

impl RoomSearch {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn render(&mut self, ui: &mut Ui) {
        ui.text_edit_singleline(&mut self.name);
        ui.collapsing("Advanced Search", |ui| {
            egui::ComboBox::from_label("Heated").selected_text(self.is_heated.to_string()).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.is_heated, SearchOpt::Any, "Any");
                ui.selectable_value(&mut self.is_heated, SearchOpt::Yes, "Yes");
                ui.selectable_value(&mut self.is_heated, SearchOpt::No, "No");
            });
            egui::ComboBox::from_label("Placed").selected_text(self.is_placed.to_string()).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.is_placed, SearchOpt::Any, "Any");
                ui.selectable_value(&mut self.is_placed, SearchOpt::Yes, "Yes");
                ui.selectable_value(&mut self.is_placed, SearchOpt::No, "No");
            });

            egui::Grid::new("grid_adv_search").striped(true).num_columns(3).show(ui, |ui| {
                ui.label("");
                ui.label("Min");
                ui.label("Max");
                ui.end_row();

                ui.label("Width");
                ui.add(egui::DragValue::new(&mut self.min_width).range(0..=self.max_width));
                ui.add(egui::DragValue::new(&mut self.max_width).range(self.min_width..=99));
                ui.end_row();
                
                ui.label("Height");
                ui.add(egui::DragValue::new(&mut self.min_height).range(0..=self.max_height));
                ui.add(egui::DragValue::new(&mut self.max_height).range(self.min_height..=99));
                ui.end_row();

                let door_order = ["Right", "Down", "Left", "Up"];
                for i in 0..4 {
                    ui.label(format!("{} Door", door_order[i]));
                    ui.add(egui::DragValue::new(&mut self.min_door_count[i]).range(0..=self.max_door_count[i]));
                    ui.add(egui::DragValue::new(&mut self.max_door_count[i]).range(self.min_door_count[i]..=9));
                    ui.end_row();
                }

                ui.label("Items");
                ui.add(egui::DragValue::new(&mut self.min_items).range(0..=self.max_items));
                ui.add(egui::DragValue::new(&mut self.max_items).range(self.min_items..=3));
                ui.end_row();
            });
        });

        if ui.button("Clear filters").clicked() {
            self.reset();
        }
    }

    pub fn filter(&self, game_data: &GameData, map: &Map) -> Vec<usize> {
        (0..game_data.room_geometry.len()).into_iter().filter(|&idx| {
            if !self.is_placed.compare(map.room_mask[idx]) {
                return false;
            }

            let room_geometry = &game_data.room_geometry[idx];
            let room_name = game_data.room_json_map[&room_geometry.room_id]["name"].as_str().unwrap();
            if !self.name.is_empty() && !room_name.to_ascii_lowercase().contains(&self.name.to_ascii_lowercase()) {
                return false;
            }
            let room_width = room_geometry.map[0].len();
            if room_width < self.min_width || room_width > self.max_width {
                return false;
            }
            let room_height = room_geometry.map.len();
            if room_height < self.min_height || room_height > self.max_height {
                return false;
            }
            if !self.is_heated.compare(room_geometry.heated) {
                return false;
            }

            let dir = ["right", "down", "left", "up"];
            for i in 0..4 {
                let door_count = room_geometry.doors.iter().filter(|door| door.direction == dir[i]).count();
                if door_count < self.min_door_count[i] || door_count > self.max_door_count[i] {
                    return false;
                }
            }

            let item_count = game_data.item_locations.iter().filter(|(room_id, _)| {
                game_data.room_idx_by_id[room_id] == idx
            }).count();
            if item_count < self.min_items || item_count > self.max_items {
                return false;
            }

            true
        }).collect()
    }
}