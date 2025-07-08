use std::{fs::File, i32, io::{Read, Write}, path::Path};

use anyhow::{anyhow, bail, Result};
use hashbrown::{HashMap, HashSet};
use maprando_game::{GameData, Map};
use serde_json::Value;
use sfml::{graphics::{Color, IntRect}, system::Vector2i, window::Key};

use crate::{utils, PlandoApp};

#[derive(PartialEq, Eq)]
pub enum SidebarMode {
    Rooms,
    Areas,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Area {
    OuterCrateria,
    InnerCrateria,
    BlueBrinstar,
    GreenBrinstar,
    PinkBrinstar,
    RedBrinstar,
    UpperNorfair,
    LowerNorfair,
    WreckedShip,
    WestMaridia,
    YellowMaridia,
    MetroidHabitat,
    MechaTourian
}

impl Area {
    pub const VALUES: [Area; 13] = [
        Area::OuterCrateria,
        Area::InnerCrateria,
        Area::BlueBrinstar,
        Area::GreenBrinstar,
        Area::PinkBrinstar,
        Area::RedBrinstar,
        Area::UpperNorfair,
        Area::LowerNorfair,
        Area::WreckedShip,
        Area::WestMaridia,
        Area::YellowMaridia,
        Area::MetroidHabitat,
        Area::MechaTourian
    ];

    pub fn to_string(&self) -> String {
        use Area::*;
        match self {
            OuterCrateria => "Outer Crateria",
            InnerCrateria => "Inner Crateria",
            BlueBrinstar => "Blue Brinstar",
            GreenBrinstar => "Green Brinstar",
            PinkBrinstar => "Pink Brinstar",
            RedBrinstar => "Red Brinstar",
            UpperNorfair => "Upper Norfair",
            LowerNorfair => "Lower Norfair",
            WreckedShip => "Wrecked Ship",
            WestMaridia => "West Maridia",
            YellowMaridia => "Yellow Maridia",
            MetroidHabitat => "Metroid Habitat",
            MechaTourian => "Mecha Tourian",
        }.to_string()
    }

    pub fn to_tuple(&self) -> (usize, usize, usize) {
        use Area::*;
        match self {
            OuterCrateria => (0, 0, 0),
            InnerCrateria => (0, 1, 0),
            BlueBrinstar => (0, 1, 1),
            GreenBrinstar => (1, 0, 0),
            PinkBrinstar => (1, 0, 1),
            RedBrinstar => (1, 1, 0),
            UpperNorfair => (2, 0, 0),
            LowerNorfair => (2, 1, 0),
            WreckedShip => (3, 0, 0),
            WestMaridia => (4, 0, 0),
            YellowMaridia => (4, 1, 0),
            MetroidHabitat => (5, 0, 0),
            MechaTourian => (5, 1, 0),
        }
    }

    pub fn to_color(&self) -> Color {
        use Area::*;
        match self {
            OuterCrateria => Color::rgb(148, 0, 222),
            InnerCrateria => Color::rgb(222, 123, 255),
            BlueBrinstar => Color::rgb(66, 0, 99),
            GreenBrinstar => Color::rgb(0, 148, 0),
            PinkBrinstar => Color::rgb(99, 206, 99),
            RedBrinstar => Color::rgb(0, 63, 0),
            UpperNorfair => Color::rgb(189, 0, 0),
            LowerNorfair => Color::rgb(255, 99, 99),
            WreckedShip => Color::rgb(132, 140, 0),
            WestMaridia => Color::rgb(25, 99, 239),
            YellowMaridia => Color::rgb(99, 165, 255),
            MetroidHabitat => Color::rgb(173, 99, 0),
            MechaTourian => Color::rgb(239, 140, 99),
        }
    }

    pub fn from_tuple(tuple: (usize, usize, usize)) -> Self {
        use Area::*;
        match tuple {
            (0, 0, _) => OuterCrateria,
            (0, 1, 0) => InnerCrateria,
            (0, 1, 1) => BlueBrinstar,
            (1, 0, 0) => GreenBrinstar,
            (1, 0, 1) => PinkBrinstar,
            (1, 1, _) => RedBrinstar,
            (2, 0, _) => UpperNorfair,
            (2, 1, _) => LowerNorfair,
            (3, _, _) => WreckedShip,
            (4, 0, _) => WestMaridia,
            (4, 1, _) => YellowMaridia,
            (5, 0, _) => MetroidHabitat,
            (5, 1, _) | _ => MechaTourian,
        }
    }
}

pub enum MapErrorType {
    DoorDisconnected(usize, usize), // (room_idx, door_idx) of door which is not connected
    AreaBounds(usize), // Area idx which exceeds boundary limits
    AreaTransitions(usize), // Number of area transition which exceeds limit
    MapPerArea(usize), // Area idx which has no map
    PhantoonMap, // Phantoon map is not connected to phantoon via exaclty one room inbetween
    PhantoonSave, // Phantoon Save is not in the same area as phantoon and his map
    ToiletNoRoom, // Toilet passes through no room
    ToiletMultipleRooms(usize, usize), // Toilet passes through at least two rooms and is not vanilla
    ToiletArea(usize, usize, usize), // (room_idx, toilet_area_idx, room_area_idx) Toilet area and the passing through room have different areas
    ToiletNoPatch(usize, i32, i32) // (room_idx, xoffset, yoffset) which the toilet passes through has no patch
}

pub struct MapEditor {
    pub selected_room_idx: Vec<usize>,
    pub selection_start: Option<Vector2i>,
    pub dragged_room_idx: Vec<usize>,
    pub dragged_room_xoffset: usize,
    pub dragged_room_yoffset: usize,

    pub room_overlaps: HashSet<(usize, usize)>,
    pub error_list: Vec<MapErrorType>,
    pub invalid_doors: HashSet<(usize, usize)>, // (room_idx, door_idx)
    pub missing_rooms: HashSet<usize>,
}

impl MapEditor {
    const AREA_MAX_WIDTH: usize = 60;
    const AREA_MAX_HEIGHT: usize = 28;
    const AREA_MAX_TRANSITIONS: usize = 23;

    pub fn new() -> MapEditor {
        MapEditor {
            selected_room_idx: Vec::new(),
            selection_start: None,
            dragged_room_idx: Vec::new(),
            dragged_room_xoffset: 0,
            dragged_room_yoffset: 0,
            room_overlaps: HashSet::new(),
            error_list: Vec::new(),
            invalid_doors: HashSet::new(),
            missing_rooms: HashSet::new(),
        }
    }

    pub fn save_map(&mut self, map: &Map, game_data: &GameData, path: &Path) -> Result<()> {
        let mut file = File::create(path)?;
        if self.is_valid(map, game_data) {
            let str = serde_json::to_string_pretty(map)?;
            file.write_all(str.as_bytes())?;
            return Ok(());
        }

        let mut data = serde_json::to_value(map)?;
        let missing_rooms = serde_json::to_value(&self.missing_rooms)?;
        data.as_object_mut().unwrap().insert("missing_rooms".to_string(), missing_rooms);
        let str = serde_json::to_string_pretty(&data)?;
        file.write_all(str.as_bytes())?;

        Ok(())
    }

    pub fn load_map(&mut self, game_data: &GameData, path: &Path) -> Result<Map> {
        let mut file = File::open(path)?;
        let mut data_str = String::new();
        file.read_to_string(&mut data_str)?;
        let mut data: Value = serde_json::from_str(&data_str)?;

        let missing_rooms = match data.as_object_mut().unwrap().remove("missing_rooms") {
            Some(value) => value.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect(),
            None => Vec::new()
        };

        let mut map: Map = serde_json::from_value(data)?;
        self.reset();

        for room in missing_rooms {
            self.erase_room(&mut map, room, game_data);
        }

        Ok(map)
    }

    pub fn is_valid(&mut self, map: &Map, game_data: &GameData) -> bool {
        self.error_list.clear();
        for &(room_idx, door_idx) in &self.invalid_doors {
            self.error_list.push(MapErrorType::DoorDisconnected(room_idx, door_idx));
        }
        self.check_area_bounds(map, game_data);
        self.check_area_transitions(map, game_data);
        self.check_toilet(map, game_data);
        self.check_map_connections(map, game_data);
        self.error_list.is_empty()
    }

    pub fn reset(&mut self) {
        self.selected_room_idx.clear();
        self.dragged_room_idx.clear();
        self.invalid_doors.clear();
        self.missing_rooms.clear();
    }

    pub fn apply_area(&mut self, map: &mut Map, room_idx: usize, area_value: Area) {
        let (area, sub_area, sub_sub_area) = area_value.to_tuple();
        map.area[room_idx] = area;
        map.subarea[room_idx] = sub_area;
        map.subsubarea[room_idx] = sub_sub_area;
    }

    pub fn swap_areas(&mut self, map: &mut Map, area1: usize, area2: usize) {
        if area1 == area2 {
            return;
        }
        for room_idx in 0..map.rooms.len() {
            let area_tuple = self.get_area_value(map, room_idx).to_tuple();
            if area_tuple.0 != area1 && area_tuple.0 != area2 {
                continue;
            }
            let other_area = if area_tuple.0 == area1 {
                (area2, area_tuple.1, area_tuple.2)
            } else {
                (area1, area_tuple.1, area_tuple.2)
            };
            let new_area = Area::from_tuple(other_area);
            self.apply_area(map, room_idx, new_area);
        }
    }

    pub fn get_area_value(&self, map: &Map, room_idx: usize) -> Area {
        let area = map.area[room_idx];
        let sub_area = map.subarea[room_idx];
        let sub_sub_area = map.subsubarea[room_idx];
        Area::from_tuple((area, sub_area, sub_sub_area))
    }

    pub fn start_drag(&mut self, map: &Map, room_idx_opt: Option<usize>, mouse_tile_x: usize, mouse_tile_y: usize, game_data: &GameData) {
        if let Some(room_idx) = room_idx_opt {
            if self.selected_room_idx.contains(&room_idx) {
                // User starts dragging on one of the selected rooms, start dragging all of them
                let bbox = self.get_selected_bbox(map, game_data).unwrap();
                self.dragged_room_idx.append(&mut self.selected_room_idx);
                self.dragged_room_xoffset = mouse_tile_x - bbox.left as usize;
                self.dragged_room_yoffset = mouse_tile_y - bbox.top as usize;
            } else {
                // User starts dragging a non-selected room, deselect and only drag this one
                let (room_x, room_y) = map.rooms[room_idx];
                self.dragged_room_idx.push(room_idx);
                self.dragged_room_xoffset = mouse_tile_x - room_x;
                self.dragged_room_yoffset = mouse_tile_y - room_y;
                self.selected_room_idx.clear();
            }
        } else {
            // No room is being dragged, start a selection
            if !(Key::LControl.is_pressed() || Key::RControl.is_pressed()) {
                self.selected_room_idx.clear();
            }
            self.selection_start = Some(Vector2i::new(mouse_tile_x as i32, mouse_tile_y as i32));
        }
    }

    pub fn stop_drag(&mut self, map: &mut Map, mouse_tile_x: usize, mouse_tile_y: usize, game_data: &GameData) {
        if !self.dragged_room_idx.is_empty() {
            // If we are dragging rooms, snap them into place
            for i in 0..self.dragged_room_idx.len() {
                self.snap_room(map, self.dragged_room_idx[i], game_data);
            }
            self.selected_room_idx.append(&mut self.dragged_room_idx);
        } else if self.selection_start.is_some() && (self.selected_room_idx.is_empty() || Key::LControl.is_pressed() || Key::RControl.is_pressed()) {
            // Otherwise we finish a selection
            let sel_pos = self.selection_start.unwrap();
            let w = mouse_tile_x as i32 - sel_pos.x;
            let h = mouse_tile_y as i32 - sel_pos.y;
            let rect = IntRect::new(sel_pos.x, sel_pos.y,w, h);
            let rect = utils::normalize_rect(rect);

            for (room_idx, &(room_x, room_y)) in map.rooms.iter().enumerate() {
                if self.missing_rooms.contains(&room_idx) {
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

    pub fn move_dragged_rooms(&mut self, map: &mut Map, mouse_tile_x: usize, mouse_tile_y: usize, game_data: &GameData) -> bool {
        if let Some(bbox) = self.get_dragged_bbox(map, game_data) {
            let left = mouse_tile_x as i32 - self.dragged_room_xoffset as i32;
            let top = mouse_tile_y as i32 - self.dragged_room_yoffset as i32;

            let left = left.max(0).min(PlandoApp::GRID_SIZE as i32 - bbox.width);
            let top = top.max(0).min(PlandoApp::GRID_SIZE as i32 - bbox.height);

            // Box hasn't moved
            if left == bbox.left && top == bbox.top {
                return false;
            }

            for &drag_idx in &self.dragged_room_idx {
                let (room_x, room_y) = map.rooms[drag_idx];
                let x_offset = room_x as i32 - bbox.left;
                let y_offset = room_y as i32 - bbox.top;

                let new_x = (left + x_offset) as usize;
                let new_y = (top + y_offset) as usize;

                map.rooms[drag_idx].0 = new_x;
                map.rooms[drag_idx].1 = new_y;
            }
            return true;
        }
        false
    }

    pub fn erase_room(&mut self, map: &mut Map, room_idx: usize, game_data: &GameData) {
        if !self.missing_rooms.insert(room_idx) {
            return;
        }
        let room_geometry = &game_data.room_geometry[room_idx];
        for (door_idx, door) in room_geometry.doors.iter().enumerate() {
            self.invalid_doors.remove(&(room_idx, door_idx));
            if let Some(other_door_conn_idx) = self.get_door_conn_idx(map, room_idx, door_idx, game_data) {
                let door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
                let prev_door_conn = map.doors.remove(other_door_conn_idx);
                let other_door_ptr_pair = if prev_door_conn.0 == door_ptr_pair { prev_door_conn.1 } else { prev_door_conn.0 };
                let invalid_door = game_data.room_and_door_idxs_by_door_ptr_pair[&other_door_ptr_pair];
                self.invalid_doors.insert(invalid_door);
            }
        }
        self.is_valid(map, game_data);
    }

    pub fn spawn_room(&mut self, map: &mut Map, room_idx: usize, game_data: &GameData) {
        self.missing_rooms.remove(&room_idx);
        self.snap_room(map, room_idx, game_data);
    }

    pub fn get_room_bounds(&self, map: &Map, room_idx: usize, game_data: &GameData) -> IntRect {
        let (room_x, room_y) = map.rooms[room_idx];
        let room_geometry = &game_data.room_geometry[room_idx];
        let room_width = room_geometry.map[0].len();
        let room_height = room_geometry.map.len();
        IntRect::new(room_x as i32, room_y as i32, room_width as i32, room_height as i32)
    }

    fn update_overlaps(&mut self, map: &Map, room_idx: usize, game_data: &GameData) {
        // Remove all overlaps with this room_idx
        self.room_overlaps.retain(|&(l, r)| l != room_idx && r != room_idx);
        if self.missing_rooms.contains(&room_idx) {
            return;
        }

        for other_idx in 0..map.rooms.len() {
            if other_idx == room_idx || self.missing_rooms.contains(&other_idx) {
                continue;
            }
            if self.check_overlap(map, room_idx, other_idx, game_data) {
                let smaller_idx = room_idx.min(other_idx);
                let bigger_idx = room_idx.max(other_idx);
                self.room_overlaps.insert((smaller_idx, bigger_idx));
                continue;
            }
        }
    }

    fn check_overlap(&self, map: &Map, room_idx: usize, other_idx: usize, game_data: &GameData) -> bool {
        let bbox = self.get_room_bounds(map, room_idx, game_data);
        if other_idx == room_idx {
            return true;
        }
        let other_bbox = self.get_room_bounds(map, other_idx, game_data);
        if let Some(intersect) = bbox.intersection(&other_bbox) {
            let (room_x, room_y) = map.rooms[room_idx];
            let (other_x, other_y) = map.rooms[other_idx];

            let map = &game_data.room_geometry[room_idx].map;
            let other_map = &game_data.room_geometry[other_idx].map;
            for y in intersect.top..(intersect.top + intersect.height) {
                for x in intersect.left..(intersect.left + intersect.width) {
                    let tile_x = x as usize - room_x;
                    let tile_y = y as usize - room_y;
                    let other_tile_x = x as usize - other_x;
                    let other_tile_y = y as usize - other_y;
                    if map[tile_y][tile_x] == 1 && other_map[other_tile_y][other_tile_x] == 1 {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn snap_room(&mut self, map: &mut Map, room_idx: usize, game_data: &GameData) {
        self.update_overlaps(map, room_idx, game_data);

        let mut orphaned_doors = HashSet::new();

        let room_geometry = &game_data.room_geometry[room_idx];
        // Invalidate all doors of moved room and all orphaned doors that were created by moving the room
        for (door_idx, door) in room_geometry.doors.iter().enumerate() {
            let cur_door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
            if let Some(prev_door_conn_idx) = self.get_door_conn_idx(map, room_idx, door_idx, game_data) {
                let prev_door_conn = map.doors[prev_door_conn_idx];
                map.doors.remove(prev_door_conn_idx);
                let other_door_ptr_pair = if prev_door_conn.0 == cur_door_ptr_pair { prev_door_conn.1 } else { prev_door_conn.0 };
                let (other_room_idx, other_door_idx) = game_data.room_and_door_idxs_by_door_ptr_pair[&other_door_ptr_pair];
                orphaned_doors.insert((other_room_idx, other_door_idx));
                self.invalid_doors.insert((other_room_idx, other_door_idx));
            }
            orphaned_doors.insert((room_idx, door_idx));
            self.invalid_doors.insert((room_idx, door_idx));
        }
        
        // Validate all orphaned doors
        while !orphaned_doors.is_empty() {
            let (room_idx, door_idx) = orphaned_doors.iter().next().unwrap().clone();
            if let Some((other_room_idx, other_door_idx)) = self.validate_door(map, room_idx, door_idx, game_data) {
                orphaned_doors.remove(&(other_room_idx, other_door_idx));
            }
            orphaned_doors.remove(&(room_idx, door_idx));
        }
    }

    fn validate_door(&mut self, map: &mut Map, room_idx: usize, door_idx: usize, game_data: &GameData) -> Option<(usize, usize)> {
        let door = &game_data.room_geometry[room_idx].doors[door_idx];
        let (dx, dy) = match door.direction.as_str() {
            "up" => (0, -1),
            "down" => (0, 1),
            "left" => (-1, 0),
            _ => (1, 0)
        };
        let dir_opposite = match door.direction.as_str() {
            "up" => "down",
            "down" => "up",
            "left" => "right",
            _ => "left"
        }.to_string();
        let (room_x, room_y) = map.rooms[room_idx];
        let target_x = door.x as i32 + room_x as i32 + dx;
        let target_y = door.y as i32 + room_y as i32 + dy;
        if target_x < 0 || target_x >= PlandoApp::GRID_SIZE as i32 || target_y < 0 || target_y >= PlandoApp::GRID_SIZE as i32 {
            self.invalid_doors.insert((room_idx, door_idx));
            return None;
        }
        let target_x = target_x as usize;
        let target_y = target_y as usize;

        for &(other_room_idx, other_door_idx) in &self.invalid_doors {
            let other_door = &game_data.room_geometry[other_room_idx].doors[other_door_idx];
            if other_door.direction != dir_opposite {
                continue;
            }
            let (room_x, room_y) = map.rooms[other_room_idx];
            let door_x = other_door.x + room_x;
            let door_y = other_door.y + room_y;
            if door_x != target_x || door_y != target_y {
                continue;
            }

            self.invalid_doors.remove(&(room_idx, door_idx));
            self.invalid_doors.remove(&(other_room_idx, other_door_idx));

            let src_ptr_pair = (door.exit_ptr, door.entrance_ptr);
            let dst_ptr_pair = (other_door.exit_ptr, other_door.entrance_ptr);
            let bidirectional = door.subtype != "sand" && other_door.subtype != "sand";
            map.doors.push((src_ptr_pair, dst_ptr_pair, bidirectional));
            return Some((other_room_idx, other_door_idx));
        }

        self.invalid_doors.insert((room_idx, door_idx));
        None
    }

    fn get_door_conn_idx(&self, map: &Map, room_idx: usize, door_idx: usize, game_data: &GameData) -> Option<usize> {
        let door = &game_data.room_geometry[room_idx].doors[door_idx];
        let door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
        map.doors.iter().position(
            |&(src, dst, _)| src == door_ptr_pair || dst == door_ptr_pair
        )
    }

    fn check_area_bounds(&mut self, map: &Map, game_data: &GameData) {
        let mut area_min = [Vector2i::new(i32::MAX, i32::MAX); 6];
        let mut area_max = [Vector2i::new(0, 0); 6];

        for (room_idx, &(room_x, room_y)) in map.rooms.iter().enumerate() {
            let area = map.area[room_idx];
            area_min[area].x = area_min[area].x.min(room_x as i32);
            area_min[area].y = area_min[area].y.min(room_y as i32);

            let room_geometry = &game_data.room_geometry[room_idx];
            let room_width = room_geometry.map[0].len();
            let room_height = room_geometry.map.len();

            let room_right = (room_x + room_width) as i32;
            let room_bottom = (room_y + room_height) as i32;
            area_max[area].x = area_max[area].x.max(room_right);
            area_max[area].y = area_max[area].y.max(room_bottom);
        }

        for idx in 0..6 {
            let area_size = area_max[idx] - area_min[idx];
            if area_size.x > Self::AREA_MAX_WIDTH as i32 || area_size.y > Self::AREA_MAX_HEIGHT as i32 {
                self.error_list.push(MapErrorType::AreaBounds(idx));
            }
        }
    }

    fn check_area_transitions(&mut self, map: &Map, game_data: &GameData) {
        let mut connection_count = 0;
        for (room_idx, room_geometry) in game_data.room_geometry.iter().enumerate() {
            for (door_idx, door) in room_geometry.doors.iter().enumerate() {
                let door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
                let door_conn_idx = match self.get_door_conn_idx(map, room_idx, door_idx, game_data) {
                    Some(idx) => idx,
                    None => {
                        self.error_list.push(MapErrorType::DoorDisconnected(room_idx, door_idx));
                        return;
                    }
                };
                let door_conn = map.doors[door_conn_idx];
                let other_door_ptr_pair = if door_conn.0 == door_ptr_pair { door_conn.1 } else { door_conn.0 };
                let (other_room_idx, _) = game_data.room_and_door_idxs_by_door_ptr_pair[&other_door_ptr_pair];

                let area = map.area[room_idx];
                let other_area = map.area[other_room_idx];
                if area != other_area {
                    connection_count += 1;
                }
                
            }
        }
        if connection_count > Self::AREA_MAX_TRANSITIONS * 2 {
            self.error_list.push(MapErrorType::AreaTransitions(connection_count / 2));
        }
    }

    fn check_toilet(&mut self, map: &Map, game_data: &GameData) {
        let (room_x, room_y) = map.rooms[game_data.toilet_room_idx];
        let toilet_bbox = IntRect::new(room_x as i32, room_y as i32 + 2, 1, 6);

        let cross_rooms: Vec<usize> = (0..map.rooms.len()).filter_map(|idx| {
            if idx == game_data.toilet_room_idx {
                return None;
            }
            let other_bbox = self.get_room_bounds(map, idx, game_data);
            if other_bbox.intersection(&toilet_bbox).is_none() {
                return None;
            }
            for y in 0..6 {
                let rel_tile_x = room_x as i32 - other_bbox.left;
                let rel_tile_y = room_y as i32 - other_bbox.top + y + 2;
                if rel_tile_x < 0 || rel_tile_x >= other_bbox.width || rel_tile_y < 0 || rel_tile_y >= other_bbox.height {
                    continue;
                }
                if game_data.room_geometry[idx].map[rel_tile_y as usize][rel_tile_x as usize] == 1 {
                    return Some(idx);
                }
            }
            None
        }).collect();

        if cross_rooms.is_empty() {
            self.error_list.push(MapErrorType::ToiletNoRoom);
            return;
        }
        if cross_rooms.len() == 2 {
            // Check for vanilla toilet intersection
            let idx_aqueduct = game_data.room_idx_by_name["Aqueduct"];
            let idx_botwoon_hallway = game_data.room_idx_by_name["Botwoon Hallway"];

            if !cross_rooms.contains(&idx_aqueduct) || !cross_rooms.contains(&idx_botwoon_hallway) {
                self.error_list.push(MapErrorType::ToiletMultipleRooms(cross_rooms[0], cross_rooms[1]));
            }

            let pos_aqueduct = map.rooms[idx_aqueduct];
            let pos_botwoon_hallway = map.rooms[idx_botwoon_hallway];

            if !(pos_aqueduct.0 + 2 == room_x && pos_aqueduct.1 == room_y + 4 && pos_botwoon_hallway.0 + 2 == room_x && pos_botwoon_hallway.1 == room_y + 3) {
                self.error_list.push(MapErrorType::ToiletMultipleRooms(cross_rooms[0], cross_rooms[1]));
            }
        }
        if cross_rooms.len() > 2 {
            self.error_list.push(MapErrorType::ToiletMultipleRooms(cross_rooms[0], cross_rooms[1]));
        }
        
        let cross_room_idx = cross_rooms[0];
        let cross_room_area = map.area[cross_room_idx];
        let toilet_area = map.area[game_data.toilet_room_idx];

        if cross_room_area != toilet_area {
            self.error_list.push(MapErrorType::ToiletArea(cross_room_idx, toilet_area, cross_room_area));
        }

        // Check if toilet patch exists
        let room_ptr = game_data.room_geometry[cross_room_idx].rom_address;
        let (toilet_x, toilet_y) = map.rooms[game_data.toilet_room_idx];
        let (room_x, room_y) = map.rooms[cross_room_idx];
        let x_offset = toilet_x as i32 - room_x as i32;
        let y_offset = toilet_y as i32 - room_y as i32;
        let patch_path = format!("../patches/mosaic/Base-{:X}-Transit-{x_offset}-{y_offset}.bps", room_ptr);

        let path = Path::new(&patch_path);
        if !path.exists() {
            self.error_list.push(MapErrorType::ToiletNoPatch(cross_room_idx, x_offset, y_offset));
        }
    }

    fn check_map_connections(&mut self, map: &Map, game_data: &GameData) {
        let mut area_maps = [false; 6];
        let map_room_idxs: Vec<usize> = game_data.room_geometry.iter().enumerate().filter_map(
            |(idx, room)| if room.name.contains(" Map Room") { Some(idx) } else { None }
        ).collect();
        // Check every area has exaclty one map station
        for room_idx in map_room_idxs {
            let area = map.area[room_idx];
            if area_maps[area] {
                self.error_list.push(MapErrorType::MapPerArea(area));
            }
            area_maps[area] = true;
        }

        // Check Phantoon Map is connected to Phantoon through one room in a singular area
        let phantoon_map_idx = game_data.room_idx_by_name["Wrecked Ship Map Room"];
        let phantoon_room_idx = game_data.room_idx_by_name["Phantoon's Room"];
        let phantoon_map_door = &game_data.room_geometry[phantoon_map_idx].doors[0];
        let phantoon_room_door = &game_data.room_geometry[phantoon_room_idx].doors[0];
        let phantoon_map_ptr_pair = (phantoon_map_door.exit_ptr, phantoon_map_door.entrance_ptr);
        let phantoon_room_ptr_pair = (phantoon_room_door.exit_ptr, phantoon_room_door.entrance_ptr);

        let phantoon_map_conn_idx = self.get_door_conn_idx(map, phantoon_map_idx, 0, game_data).unwrap();
        let phantoon_room_conn_idx = self.get_door_conn_idx(map, phantoon_room_idx, 0, game_data).unwrap();
        let phantoon_map_conn = map.doors[phantoon_map_conn_idx];
        let phantoon_room_conn = map.doors[phantoon_room_conn_idx];

        let other_map_ptr_pair = if phantoon_map_conn.0 == phantoon_map_ptr_pair { phantoon_map_conn.1 } else { phantoon_map_conn.0 };
        let other_room_ptr_pair = if phantoon_room_conn.0 == phantoon_room_ptr_pair { phantoon_room_conn.1 } else { phantoon_room_conn.0 };

        let other_map_room = game_data.room_and_door_idxs_by_door_ptr_pair[&other_map_ptr_pair];
        let other_room_room = game_data.room_and_door_idxs_by_door_ptr_pair[&other_room_ptr_pair];

        if other_map_room.0 != other_room_room.0 {
            self.error_list.push(MapErrorType::PhantoonMap);
        }
        let area_phantoon = map.area[phantoon_room_idx];
        let area_map = map.area[phantoon_map_idx];
        let area_other = map.area[other_map_room.0];
        if area_phantoon != area_map || area_map != area_other {
            self.error_list.push(MapErrorType::PhantoonMap);
        }

        // Check if Phantoon's Save is in the same Area as Phantoon
        let phantoon_save_idx = game_data.room_idx_by_name["Wrecked Ship Save Room"];
        let area_save = map.area[phantoon_save_idx];
        if area_save != area_phantoon {
            self.error_list.push(MapErrorType::PhantoonSave);
        }
    }
}



fn index_to_area_name(idx: usize) -> String {
    match idx {
        0 => "Crateria",
        1 => "Brinstar",
        2 => "Norfair",
        3 => "Wrecked Ship",
        4 => "Maridia",
        _ => "Tourian"
    }.to_string()
}