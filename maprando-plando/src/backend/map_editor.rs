use std::{fs::File, i32, io::{Read, Write}, path::Path};

use anyhow::Result;
use hashbrown::{HashMap, HashSet};
use maprando_game::{GameData, Map};
use serde_json::Value;
use sfml::{graphics::{Color, IntRect}, system::Vector2i};

use crate::PlandoApp;

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

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MapErrorType {
    DoorDisconnected(usize, usize), // (room_idx, door_idx) of door which is not connected
    AreaBounds(usize, usize, usize), // Area idx which exceeds boundary limits followed by current (width, height)
    AreaTransitions(usize), // Number of area transition which exceeds limit
    MapPerArea(usize), // room_idx of a double map
    MapBounds(i32, i32, usize, usize), // Map exceeds boundary limits with current size (x, y, width, height)
    PhantoonMap, // Phantoon map is not connected to phantoon via exaclty one room inbetween
    PhantoonSave, // Phantoon Save is not in the same area as phantoon and his map
    ToiletNoRoom, // Toilet passes through no room
    ToiletMultipleRooms(usize, usize), // Toilet passes through at least two rooms and is not vanilla
    ToiletArea(usize, usize, usize), // (room_idx, toilet_area_idx, room_area_idx) Toilet area and the passing through room have different areas
    ToiletNoPatch(usize, i32, i32, Option<(i32, i32)>) // (room_idx, xoffset, yoffset, alternative) which the toilet passes through has no patch
}

impl MapErrorType {
    pub fn to_string(&self, game_data: &GameData) -> String {
        match self {
            MapErrorType::DoorDisconnected(_, _) => format!("Door is not connected"),
            MapErrorType::AreaBounds(_, w, h) =>
                format!("Area exceeds maximum size: Currently ({w}, {h}), Maximum: ({}, {})", MapEditor::AREA_MAX_WIDTH, MapEditor::AREA_MAX_HEIGHT),
            MapErrorType::AreaTransitions(t) =>
                format!("Number of maximum area transitions exceeded: Currently {t}, Maximum {}", MapEditor::AREA_MAX_TRANSITIONS),
            MapErrorType::MapPerArea(_) => 
                format!("This map already has a Map Station"),
            MapErrorType::MapBounds(_, _, w, h) => 
                format!("Map exceeds maximum size: Currently ({w}, {h}), Maximum: ({}, {})", MapEditor::MAP_MAX_SIZE, MapEditor::MAP_MAX_SIZE),
            MapErrorType::PhantoonMap => 
                format!("Phantoon Map Station has to be connected to Phantoon's Room via one intermediate room, all of the same area"),
            MapErrorType::PhantoonSave => 
                format!("Wrecked Ship Map Station has to be in the same area as Phantoon's Room"),
            MapErrorType::ToiletNoRoom => 
                format!("Toilet has to cross exactly one room. Currently crossing no room"),
            MapErrorType::ToiletMultipleRooms(idx1, idx2) => {
                let room_name1 = &game_data.room_geometry[*idx1].name;
                let room_name2 = &game_data.room_geometry[*idx2].name;
                format!("Toilet has to cross exactly one room. Currently ({room_name1} and {room_name2})")
            }
            MapErrorType::ToiletArea(_, _, _) => 
                format!("Toilet has to be of the same area as the room it is crossing"),
            MapErrorType::ToiletNoPatch(idx, x_offset, y_offset, alternative) => {
                let room_name = &game_data.room_geometry[*idx].name;
                match alternative {
                    Some((x, y)) => format!("There is no mosaic patch for the toilet crossing {room_name} at offset ({x_offset}, {y_offset}). A possible offset would be ({x}, {y})"),
                    None => format!("There is no mosaic patch for the toilet crossing {room_name}")
                }
            }
        }
    }
}

pub struct MapEditor {
    map: Map,

    toilet_patch_map: HashMap<usize, Vec<(i32, i32)>>,

    pub room_overlaps: HashSet<(usize, usize)>,
    pub error_list: Vec<MapErrorType>,
    pub invalid_doors: HashSet<(usize, usize)>, // (room_idx, door_idx)
    pub missing_rooms: HashSet<usize>,
}

impl MapEditor {
    pub const AREA_MAX_WIDTH: usize = 60;
    pub const AREA_MAX_HEIGHT: usize = 28;
    pub const AREA_MAX_TRANSITIONS: usize = 23;
    pub const MAP_MAX_SIZE: usize = 72;

    pub fn new(map: Map) -> MapEditor {
        MapEditor {
            map,
            toilet_patch_map: Self::generate_toilet_map().unwrap_or_default(),
            room_overlaps: HashSet::new(),
            error_list: Vec::new(),
            invalid_doors: HashSet::new(),
            missing_rooms: HashSet::new(),
        }
    }

    fn generate_toilet_map() -> Result<HashMap<usize, Vec<(i32, i32)>>> {
        let mut toilet_patch_map: HashMap<usize, Vec<(i32, i32)>> = HashMap::new();

        let dir_path = Path::new("../patches/mosaic");
        let patches: Vec<_> = std::fs::read_dir(dir_path)?.filter_map(|path| {
            path.ok().map(|path| path.file_name().into_string().unwrap().trim_end_matches(".bps").to_string())
        }).collect();

        for patch in patches {
            let split: Vec<&str> = patch.splitn(5, '-').collect();

            if split.len() < 5 { continue; }
            if split[0] != "Base" || split[2] != "Transit" { continue; }
            let room_ptr = match usize::from_str_radix(split[1], 16) {
                Ok(idx) => idx,
                Err(_) => continue
            };
            let x_offset: i32 = match split[3].parse() {
                Ok(num) => num,
                Err(_) => continue
            };
            let y_offset: i32 = match split[4].parse() {
                Ok(num) => num,
                Err(_) => continue
            };

            match toilet_patch_map.get_mut(&room_ptr) {
                Some(vec) => vec.push((x_offset, y_offset)),
                None => { toilet_patch_map.insert(room_ptr, vec![(x_offset, y_offset)]); }
            };
        }

        Ok(toilet_patch_map)
    }

    pub fn get_map(&self) -> &Map {
        &self.map
    }

    pub fn move_room(&mut self, room_idx: usize, x: usize, y: usize) {
        self.map.rooms[room_idx] = (x, y);
    }

    pub fn get_room_at(&self, x: usize, y: usize, game_data: &GameData) -> Option<usize> {
        self.map.rooms.iter().enumerate().position(|(room_idx, &(room_x, room_y))| {
            let room_geometry = &game_data.room_geometry[room_idx];
            let room_width = room_geometry.map[0].len();
            let room_height = room_geometry.map.len();
            x >= room_x && y >= room_y && x < room_x + room_width && y < room_y + room_height
        })
    }

    pub fn save_map(&mut self, game_data: &GameData, path: &Path) -> Result<()> {
        let mut file = File::create(path)?;
        if self.is_valid(game_data) {
            let str = serde_json::to_string_pretty(&self.map)?;
            file.write_all(str.as_bytes())?;
            return Ok(());
        }

        let mut data = serde_json::to_value(&self.map)?;
        let missing_rooms = serde_json::to_value(&self.missing_rooms)?;
        data.as_object_mut().unwrap().insert("missing_rooms".to_string(), missing_rooms);
        let str = serde_json::to_string_pretty(&data)?;
        file.write_all(str.as_bytes())?;

        Ok(())
    }

    pub fn load_map(&mut self, map: Map, game_data: &GameData) {
        self.reset();
        self.map = map;
        self.is_valid(game_data);
    }

    pub fn load_map_from_file(&mut self, game_data: &GameData, path: &Path) -> Result<()> {
        let mut file = File::open(path)?;
        let mut data_str = String::new();
        file.read_to_string(&mut data_str)?;
        let mut data: Value = serde_json::from_str(&data_str)?;

        let missing_rooms = match data.as_object_mut().unwrap().remove("missing_rooms") {
            Some(value) => value.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect(),
            None => Vec::new()
        };

        self.map = serde_json::from_value(data)?;
        self.reset();

        for room in missing_rooms {
            self.erase_room(room, game_data);
        }

        self.is_valid(game_data);

        Ok(())
    }

    pub fn is_valid(&mut self, game_data: &GameData) -> bool {
        self.error_list.clear();
        for &(room_idx, door_idx) in &self.invalid_doors {
            self.error_list.push(MapErrorType::DoorDisconnected(room_idx, door_idx));
        }
        self.check_area_bounds(game_data);
        self.check_map_bounds(game_data);
        self.check_area_transitions(game_data);
        self.check_toilet(game_data);
        self.check_map_connections(game_data);
        self.error_list.is_empty()
    }

    pub fn reset(&mut self) {
        self.invalid_doors.clear();
        self.missing_rooms.clear();
        self.error_list.clear();
    }

    /*pub fn apply_area(&mut self, room_idx: usize, area_value: Area) {
        let (area, sub_area, sub_sub_area) = area_value.to_tuple();
        self.map.area[room_idx] = area;
        self.map.subarea[room_idx] = sub_area;
        self.map.subsubarea[room_idx] = sub_sub_area;
    }

    pub fn swap_areas(&mut self, area1: usize, area2: usize) {
        if area1 == area2 {
            return;
        }
        for room_idx in 0..self.map.rooms.len() {
            let area_tuple = self.get_area_value(room_idx).to_tuple();
            if area_tuple.0 != area1 && area_tuple.0 != area2 {
                continue;
            }
            let other_area = if area_tuple.0 == area1 {
                (area2, area_tuple.1, area_tuple.2)
            } else {
                (area1, area_tuple.1, area_tuple.2)
            };
            let new_area = Area::from_tuple(other_area);
            self.apply_area(room_idx, new_area);
        }
    }

    pub fn get_area_value(&self, room_idx: usize) -> Area {
        let area = self.map.area[room_idx];
        let sub_area = self.map.subarea[room_idx];
        let sub_sub_area = self.map.subsubarea[room_idx];
        Area::from_tuple((area, sub_area, sub_sub_area))
    }*/

    pub fn erase_room(&mut self, room_idx: usize, game_data: &GameData) {
        if !self.missing_rooms.insert(room_idx) {
            return;
        }
        let room_geometry = &game_data.room_geometry[room_idx];
        for (door_idx, door) in room_geometry.doors.iter().enumerate() {
            self.invalid_doors.remove(&(room_idx, door_idx));
            if let Some(other_door_conn_idx) = self.get_door_conn_idx(room_idx, door_idx, game_data) {
                let door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
                let prev_door_conn = self.map.doors.remove(other_door_conn_idx);
                let other_door_ptr_pair = if prev_door_conn.0 == door_ptr_pair { prev_door_conn.1 } else { prev_door_conn.0 };
                let invalid_door = game_data.room_and_door_idxs_by_door_ptr_pair[&other_door_ptr_pair];
                self.invalid_doors.insert(invalid_door);
            }
        }
        self.is_valid(game_data);
    }

    pub fn spawn_room(&mut self, room_idx: usize, game_data: &GameData) {
        self.missing_rooms.remove(&room_idx);
        self.snap_room(room_idx, game_data);
    }

    pub fn get_room_bounds(&self, room_idx: usize, game_data: &GameData) -> IntRect {
        let (room_x, room_y) = self.map.rooms[room_idx];
        let room_geometry = &game_data.room_geometry[room_idx];
        let room_width = room_geometry.map[0].len();
        let room_height = room_geometry.map.len();
        IntRect::new(room_x as i32, room_y as i32, room_width as i32, room_height as i32)
    }

    fn update_overlaps(&mut self, room_idx: usize, game_data: &GameData) {
        // Remove all overlaps with this room_idx
        self.room_overlaps.retain(|&(l, r)| l != room_idx && r != room_idx);
        if self.missing_rooms.contains(&room_idx) {
            return;
        }

        for other_idx in 0..self.map.rooms.len() {
            if other_idx == room_idx || self.missing_rooms.contains(&other_idx) {
                continue;
            }
            if self.check_overlap(room_idx, other_idx, game_data) {
                let smaller_idx = room_idx.min(other_idx);
                let bigger_idx = room_idx.max(other_idx);
                self.room_overlaps.insert((smaller_idx, bigger_idx));
                continue;
            }
        }
    }

    fn check_overlap(&self, room_idx: usize, other_idx: usize, game_data: &GameData) -> bool {
        let bbox = self.get_room_bounds(room_idx, game_data);
        if other_idx == room_idx {
            return true;
        }
        let other_bbox = self.get_room_bounds(other_idx, game_data);
        if let Some(intersect) = bbox.intersection(&other_bbox) {
            let (room_x, room_y) = self.map.rooms[room_idx];
            let (other_x, other_y) = self.map.rooms[other_idx];

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

    pub fn snap_room(&mut self, room_idx: usize, game_data: &GameData) {
        self.update_overlaps(room_idx, game_data);

        let mut orphaned_doors = HashSet::new();

        let room_geometry = &game_data.room_geometry[room_idx];
        // Invalidate all doors of moved room and all orphaned doors that were created by moving the room
        for (door_idx, door) in room_geometry.doors.iter().enumerate() {
            let cur_door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
            if let Some(prev_door_conn_idx) = self.get_door_conn_idx(room_idx, door_idx, game_data) {
                let prev_door_conn = self.map.doors[prev_door_conn_idx];
                self.map.doors.remove(prev_door_conn_idx);
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
            if let Some((other_room_idx, other_door_idx)) = self.validate_door(room_idx, door_idx, game_data) {
                orphaned_doors.remove(&(other_room_idx, other_door_idx));
            }
            orphaned_doors.remove(&(room_idx, door_idx));
        }

        self.is_valid(game_data);
    }

    fn validate_door(&mut self, room_idx: usize, door_idx: usize, game_data: &GameData) -> Option<(usize, usize)> {
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
        let (room_x, room_y) = self.map.rooms[room_idx];
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
            let (room_x, room_y) = self.map.rooms[other_room_idx];
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
            self.map.doors.push((src_ptr_pair, dst_ptr_pair, bidirectional));
            return Some((other_room_idx, other_door_idx));
        }

        self.invalid_doors.insert((room_idx, door_idx));
        None
    }

    fn get_door_conn_idx(&self, room_idx: usize, door_idx: usize, game_data: &GameData) -> Option<usize> {
        let door = &game_data.room_geometry[room_idx].doors[door_idx];
        let door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
        self.map.doors.iter().position(
            |&(src, dst, _)| src == door_ptr_pair || dst == door_ptr_pair
        )
    }

    fn check_area_bounds(&mut self, game_data: &GameData) {
        let mut area_min = [Vector2i::new(i32::MAX, i32::MAX); 6];
        let mut area_max = [Vector2i::new(0, 0); 6];

        for (room_idx, &(room_x, room_y)) in self.map.rooms.iter().enumerate() {
            let area = self.map.area[room_idx];
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
                self.error_list.push(MapErrorType::AreaBounds(idx, area_size.x as usize, area_size.y as usize));
            }
        }
    }

    fn check_map_bounds(&mut self, game_data: &GameData) {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = 0;
        let mut max_y = 0;

        for (room_idx, &(room_x, room_y)) in self.map.rooms.iter().enumerate() {
            let room_geometry = &game_data.room_geometry[room_idx];
            let room_width = room_geometry.map[0].len();
            let room_height = room_geometry.map.len();

            let room_right = (room_x + room_width) as i32;
            let room_bottom = (room_y + room_height) as i32;

            min_x = min_x.min(room_x as i32);
            min_y = min_y.min(room_y as i32);
            max_x = max_x.max(room_right);
            max_y = max_y.max(room_bottom);
        }

        let map_width = (max_x - min_x) as usize;
        let map_height = (max_y - min_y) as usize;
        if map_width > Self::MAP_MAX_SIZE || map_height > Self::MAP_MAX_SIZE {
            self.error_list.push(MapErrorType::MapBounds(min_x, min_y, map_width, map_height));
        }
    }

    fn check_area_transitions(&mut self, game_data: &GameData) {
        let mut connection_count = 0;
        for (room_idx, room_geometry) in game_data.room_geometry.iter().enumerate() {
            for (door_idx, door) in room_geometry.doors.iter().enumerate() {
                let door_ptr_pair = (door.exit_ptr, door.entrance_ptr);
                let door_conn_idx = match self.get_door_conn_idx(room_idx, door_idx, game_data) {
                    Some(idx) => idx,
                    None => {
                        self.error_list.push(MapErrorType::DoorDisconnected(room_idx, door_idx));
                        return;
                    }
                };
                let door_conn = self.map.doors[door_conn_idx];
                let other_door_ptr_pair = if door_conn.0 == door_ptr_pair { door_conn.1 } else { door_conn.0 };
                let (other_room_idx, _) = game_data.room_and_door_idxs_by_door_ptr_pair[&other_door_ptr_pair];

                let area = self.map.area[room_idx];
                let other_area = self.map.area[other_room_idx];
                if area != other_area {
                    connection_count += 1;
                }
                
            }
        }
        if connection_count > Self::AREA_MAX_TRANSITIONS * 2 {
            self.error_list.push(MapErrorType::AreaTransitions(connection_count / 2));
        }
    }

    fn check_toilet(&mut self, game_data: &GameData) {
        let (room_x, room_y) = self.map.rooms[game_data.toilet_room_idx];
        let toilet_bbox = IntRect::new(room_x as i32, room_y as i32 + 2, 1, 6);

        let cross_rooms: Vec<usize> = (0..self.map.rooms.len()).filter_map(|idx| {
            if idx == game_data.toilet_room_idx {
                return None;
            }
            let other_bbox = self.get_room_bounds(idx, game_data);
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
            let idx_aqueduct = game_data.room_idx_by_ptr[&513447];
            let idx_botwoon_hallway = game_data.room_idx_by_ptr[&513559];

            if !cross_rooms.contains(&idx_aqueduct) || !cross_rooms.contains(&idx_botwoon_hallway) {
                self.error_list.push(MapErrorType::ToiletMultipleRooms(cross_rooms[0], cross_rooms[1]));
            }

            let pos_aqueduct = self.map.rooms[idx_aqueduct];
            let pos_botwoon_hallway = self.map.rooms[idx_botwoon_hallway];

            if !(pos_aqueduct.0 + 2 == room_x && pos_aqueduct.1 == room_y + 4 && pos_botwoon_hallway.0 + 2 == room_x && pos_botwoon_hallway.1 == room_y + 3) {
                self.error_list.push(MapErrorType::ToiletMultipleRooms(cross_rooms[0], cross_rooms[1]));
            } else {
                // Toilet is vanilla, don't check for patches
                return;
            }
        }
        if cross_rooms.len() > 2 {
            self.error_list.push(MapErrorType::ToiletMultipleRooms(cross_rooms[0], cross_rooms[1]));
        }
        
        let cross_room_idx = cross_rooms[0];
        let cross_room_area = self.map.area[cross_room_idx];
        let toilet_area = self.map.area[game_data.toilet_room_idx];

        if cross_room_area != toilet_area {
            self.error_list.push(MapErrorType::ToiletArea(cross_room_idx, toilet_area, cross_room_area));
        }

        // Check if toilet patch exists
        let room_ptr = game_data.room_geometry[cross_room_idx].rom_address;
        let (toilet_x, toilet_y) = self.map.rooms[game_data.toilet_room_idx];
        let (room_x, room_y) = self.map.rooms[cross_room_idx];
        let x_offset = toilet_x as i32 - room_x as i32;
        let y_offset = toilet_y as i32 - room_y as i32;

        let offsets = match self.toilet_patch_map.get(&room_ptr) {
            Some(vec) => vec,
            None => {
                self.error_list.push(MapErrorType::ToiletNoPatch(cross_room_idx, x_offset, y_offset, None));
                return;
            }
        };

        let min_dist_offset = *offsets.iter().min_by_key(|&&(x, y)| {
            (x - x_offset).abs() + (y - y_offset).abs()
        }).unwrap();

        if min_dist_offset != (x_offset, y_offset) {
            self.error_list.push(MapErrorType::ToiletNoPatch(cross_room_idx, x_offset, y_offset, Some(min_dist_offset)));
        }
    }

    fn check_map_connections(&mut self, game_data: &GameData) {
        let mut area_maps = [false; 6];
        let map_room_idxs: Vec<usize> = game_data.room_geometry.iter().enumerate().filter_map(
            |(idx, room)| if room.name.contains(" Map Room") { Some(idx) } else { None }
        ).collect();
        // Check every area has exaclty one map station
        for room_idx in map_room_idxs {
            let area = self.map.area[room_idx];
            if area_maps[area] {
                self.error_list.push(MapErrorType::MapPerArea(area));
            }
            area_maps[area] = true;
        }

        // Check Phantoon Map is connected to Phantoon through one room in a singular area
        let phantoon_map_idx = game_data.room_idx_by_ptr[&511179];
        let phantoon_room_idx = game_data.room_idx_by_ptr[&511251];
        let phantoon_map_door = &game_data.room_geometry[phantoon_map_idx].doors[0];
        let phantoon_room_door = &game_data.room_geometry[phantoon_room_idx].doors[0];
        let phantoon_map_ptr_pair = (phantoon_map_door.exit_ptr, phantoon_map_door.entrance_ptr);
        let phantoon_room_ptr_pair = (phantoon_room_door.exit_ptr, phantoon_room_door.entrance_ptr);

        let phantoon_map_conn_idx = self.get_door_conn_idx(phantoon_map_idx, 0, game_data).unwrap();
        let phantoon_room_conn_idx = self.get_door_conn_idx(phantoon_room_idx, 0, game_data).unwrap();
        let phantoon_map_conn = self.map.doors[phantoon_map_conn_idx];
        let phantoon_room_conn = self.map.doors[phantoon_room_conn_idx];

        let other_map_ptr_pair = if phantoon_map_conn.0 == phantoon_map_ptr_pair { phantoon_map_conn.1 } else { phantoon_map_conn.0 };
        let other_room_ptr_pair = if phantoon_room_conn.0 == phantoon_room_ptr_pair { phantoon_room_conn.1 } else { phantoon_room_conn.0 };

        let other_map_room = game_data.room_and_door_idxs_by_door_ptr_pair[&other_map_ptr_pair];
        let other_room_room = game_data.room_and_door_idxs_by_door_ptr_pair[&other_room_ptr_pair];

        if other_map_room.0 != other_room_room.0 {
            self.error_list.push(MapErrorType::PhantoonMap);
        }
        let area_phantoon = self.map.area[phantoon_room_idx];
        let area_map = self.map.area[phantoon_map_idx];
        let area_other = self.map.area[other_map_room.0];
        if area_phantoon != area_map || area_map != area_other {
            self.error_list.push(MapErrorType::PhantoonMap);
        }

        // Check if Phantoon's Save is in the same Area as Phantoon
        let phantoon_save_idx = game_data.room_idx_by_ptr[&511626];
        let area_save = self.map.area[phantoon_save_idx];
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