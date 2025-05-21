use std::{fs::File, io::Write, path::Path};

use anyhow::Result;
use hashbrown::HashSet;
use maprando_game::{GameData, Map};

use crate::PlandoApp;

#[derive(PartialEq, Eq)]
pub enum SidebarMode {
    Rooms,
    Areas,
    SubAreas,
}

pub struct MapEditor {
    pub map: Map,
    pub dragged_room_idx: Option<usize>,
    pub dragged_room_xoffset: usize,
    pub dragged_room_yoffset: usize,

    pub invalid_doors: HashSet<(usize, usize)>, // (room_idx, door_idx)
    pub missing_rooms: HashSet<usize>, // room_idx

    pub sidebar_mode: SidebarMode,
    pub search_str: String,
}

impl MapEditor {
    pub fn new(map: Map) -> MapEditor {
        MapEditor {
            map,
            dragged_room_idx: None,
            dragged_room_xoffset: 0,
            dragged_room_yoffset: 0,
            invalid_doors: HashSet::new(),
            missing_rooms: HashSet::new(),
            sidebar_mode: SidebarMode::Rooms,
            search_str: String::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.invalid_doors.is_empty() && self.missing_rooms.is_empty()
    }

    pub fn reset(&mut self, map: Map) {
        self.map = map;
        self.dragged_room_idx = None;
        self.invalid_doors.clear();
        self.missing_rooms.clear();
    }

    pub fn erase_room(&mut self, room_idx: usize, game_data: &GameData) {
        self.missing_rooms.insert(room_idx);
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
    }

    pub fn spawn_room(&mut self, room_idx: usize, game_data: &GameData) {
        self.missing_rooms.remove(&room_idx);
        self.snap_room(room_idx, game_data);
    }

    pub fn snap_room(&mut self, room_idx: usize, game_data: &GameData) {
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

    pub fn save_map(&self, path: &Path) -> Result<()> {
        let str = serde_json::to_string_pretty(&self.map)?;
        let mut file = File::create(path)?;
        file.write_all(str.as_bytes())?;
        Ok(())
    }
}