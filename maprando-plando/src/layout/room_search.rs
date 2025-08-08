use maprando_game::GameData;

#[derive(PartialEq)]
pub enum SearchOpt {
    Any, Yes, No
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
        }
    }
}

impl RoomSearch {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn filter(&self, game_data: &GameData) -> Vec<usize> {
        (0..game_data.room_geometry.len()).into_iter().filter(|&idx| {
            let room_geometry = &game_data.room_geometry[idx];
            if !self.name.is_empty() && !room_geometry.name.to_ascii_lowercase().contains(&self.name.to_ascii_lowercase()) {
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
            if self.is_heated != SearchOpt::Any && (self.is_heated == SearchOpt::Yes) != room_geometry.heated {
                return false;
            }

            let dir = ["right", "down", "left", "up"];
            for i in 0..4 {
                let door_count = room_geometry.doors.iter().filter(|door| door.direction == dir[i]).count();
                if door_count < self.min_door_count[i] || door_count > self.max_door_count[i] {
                    return false;
                }
            }

            true
        }).collect()
    }
}