use maprando::randomize::LockedDoor;
use maprando_game::{GameData, Map};
use maprando_plando_backend::map_editor::MapEditor;
use sfml::{graphics::IntRect, system::Vector2i, window::Key};

use crate::utils;

#[derive(Default)]
pub struct MapEditorUi {
    pub selected_room_idx: Vec<usize>,
    pub selection_start: Option<Vector2i>,
    pub dragged_room_idx: Vec<usize>,
    pub dragged_room_xoffset: usize,
    pub dragged_room_yoffset: usize,

    pub swap_first: usize,
    pub swap_second: usize
}

impl MapEditorUi {
    pub fn start_drag(&mut self, map: &Map, room_idx_opt: Option<usize>, mouse_tile_x: usize, mouse_tile_y: usize, game_data: &GameData) {
        if let Some(room_idx) = room_idx_opt && map.room_mask[room_idx] {
            if self.selected_room_idx.contains(&room_idx) {
                // User starts dragging on one of the selected rooms, start dragging all of them
                let bbox = self.get_selected_bbox(map, game_data).unwrap();
                self.dragged_room_idx.append(&mut self.selected_room_idx);
                self.dragged_room_xoffset = mouse_tile_x - bbox.left as usize;
                self.dragged_room_yoffset = mouse_tile_y - bbox.top as usize;
            } else {
                // User starts dragging a non-selected room, deselect and only drag this one
                self.dragged_room_idx.push(room_idx);
                if Key::LControl.is_pressed() || Key::RControl.is_pressed() {
                    self.dragged_room_idx.append(&mut self.selected_room_idx);
                } else {
                    self.selected_room_idx.clear();
                }
                let bbox = self.get_dragged_bbox(map, game_data).unwrap();
                self.dragged_room_xoffset = mouse_tile_x - bbox.left as usize;
                self.dragged_room_yoffset = mouse_tile_y - bbox.top as usize;
            }
        } else {
            // No room is being dragged, start a selection
            if !(Key::LControl.is_pressed() || Key::RControl.is_pressed()) {
                self.selected_room_idx.clear();
            }
            self.selection_start = Some(Vector2i::new(mouse_tile_x as i32, mouse_tile_y as i32));
        }
    }

    pub fn stop_drag(&mut self, map_editor: &mut MapEditor, mouse_tile_x: usize, mouse_tile_y: usize, game_data: &GameData, locked_doors: &Vec<LockedDoor>) {
        if !self.dragged_room_idx.is_empty() {
            // If we are dragging rooms, snap them into place
            for i in 0..self.dragged_room_idx.len() {
                map_editor.snap_room(self.dragged_room_idx[i], locked_doors);
            }
            self.selected_room_idx.append(&mut self.dragged_room_idx);
        } else if self.selection_start.is_some() && (self.selected_room_idx.is_empty() || Key::LControl.is_pressed() || Key::RControl.is_pressed()) {
            // Otherwise we finish a selection
            let sel_pos = self.selection_start.unwrap();
            let w = mouse_tile_x as i32 - sel_pos.x;
            let h = mouse_tile_y as i32 - sel_pos.y;
            let rect = IntRect::new(sel_pos.x, sel_pos.y,w, h);
            let rect = utils::normalize_rect(rect);

            for (room_idx, &(room_x, room_y)) in map_editor.get_map().rooms.iter().enumerate() {
                if !map_editor.get_map().room_mask[room_idx] {
                    continue;
                }

                let room_geometry = &game_data.room_geometry[room_idx];
                let room_width = room_geometry.map[0].len();
                let room_height = room_geometry.map.len();
                let room_rect = IntRect::new(room_x as i32, room_y as i32, room_width as i32, room_height as i32);

                if let Some(intersect) = rect.intersection(&room_rect) {
                    if intersect == room_rect {
                        self.selected_room_idx.push(room_idx);
                    }
                }
            }

            self.selected_room_idx.sort();
            self.selected_room_idx.dedup();
            self.selection_start = None;
        }
    }

    pub fn get_selected_bbox(&self, map: &Map, game_data: &GameData) -> Option<IntRect> {
        self.get_bbox(map, game_data, &self.selected_room_idx)
    }

    pub fn get_dragged_bbox(&self, map: &Map, game_data: &GameData) -> Option<IntRect> {
        self.get_bbox(map, game_data, &self.dragged_room_idx)
    }

    fn get_bbox(&self, map: &Map, game_data: &GameData, vec: &Vec<usize>) -> Option<IntRect> {
        vec.iter().map(|&idx| {
            let room_geometry = &game_data.room_geometry[idx];
            let (room_x, room_y) = map.rooms[idx];
            let room_width = room_geometry.map[0].len();
            let room_height = room_geometry.map.len();
            IntRect::new(room_x as i32, room_y as i32, room_width as i32, room_height as i32)
        }).reduce(|accum, elem| {
            let left = accum.left.min(elem.left);
            let top = accum.top.min(elem.top);
            let right = (accum.left + accum.width).max(elem.left + elem.width);
            let bottom = (accum.top + accum.height).max(elem.top + elem.height);
            IntRect::new(left, top, right - left, bottom - top)
        })
    }

    pub fn move_dragged_rooms(&mut self, map_editor: &mut MapEditor, mouse_tile_x: usize, mouse_tile_y: usize, game_data: &GameData) -> bool {
        if let Some(bbox) = self.get_dragged_bbox(map_editor.get_map(), game_data) {
            let left = mouse_tile_x as i32 - self.dragged_room_xoffset as i32;
            let top = mouse_tile_y as i32 - self.dragged_room_yoffset as i32;

            let left = left.max(0);
            let top = top.max(0);

            // Box hasn't moved
            if left == bbox.left && top == bbox.top {
                return false;
            }

            for &drag_idx in &self.dragged_room_idx {
                let (room_x, room_y) = map_editor.get_map().rooms[drag_idx];
                let x_offset = room_x as i32 - bbox.left;
                let y_offset = room_y as i32 - bbox.top;

                let new_x = (left + x_offset) as usize;
                let new_y = (top + y_offset) as usize;

                map_editor.move_room(drag_idx, new_x, new_y);
            }
            return true;
        }
        false
    }
}