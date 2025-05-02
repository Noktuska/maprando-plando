use std::path::Path;

use anyhow::{anyhow, Result};
use hashbrown::HashSet;
use maprando::{map_repository::MapRepository, settings::{RandomizerSettings, WallJump}};
use maprando_game::{DoorPtrPair, GameData, Item, Map, StartLocation};
use rand::{RngCore, SeedableRng};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DoubleItemPlacement {
    Middle, Left, Right
}

#[derive(Clone)]
pub struct ItemPlacement {
    pub item: Item,
    pub room_idx: usize,
    pub tile_x: usize,
    pub tile_y: usize,
    pub double_item_placement: DoubleItemPlacement
}

pub const ITEM_VALUES: [Item; 23] = [
    Item::ETank, Item::Missile, Item::Super, Item::PowerBomb, Item::Bombs, Item::Charge, Item::Ice, Item::HiJump, Item::SpeedBooster,
    Item::Wave, Item::Spazer, Item::SpringBall, Item::Varia, Item::Gravity, Item::XRayScope, Item::Plasma, Item::Grapple,
    Item::SpaceJump, Item::ScrewAttack, Item::Morph, Item::ReserveTank, Item::WallJump, Item::Nothing
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Placeable {
    Helm = 0,
    ETank,
    Missile,
    SuperMissile,
    PowerBomb,
    Bombs,
    Charge,
    Ice,
    HighJump,
    SpeedBooster,
    Wave,
    Spazer,
    Springball,
    Varia,
    Gravity,
    XRay,
    Plasma,
    Grapple,
    SpaceJump,
    ScrewAttack,
    Morph,
    ReserveTank,
    WalljumpBoots,
    DoorMissile,
    DoorSuper,
    DoorPowerBomb,
    DoorCharge,
    DoorIce,
    DoorWave,
    DoorSpazer,
    DoorPlasma
}

impl Placeable {
    pub const VALUES: [Self; 31] = [Self::Helm,
    Self::ETank,
    Self::Missile,
    Self::SuperMissile,
    Self::PowerBomb,
    Self::Bombs,
    Self::Charge,
    Self::Ice,
    Self::HighJump,
    Self::SpeedBooster,
    Self::Wave,
    Self::Spazer,
    Self::Springball,
    Self::Varia,
    Self::Gravity,
    Self::XRay,
    Self::Plasma,
    Self::Grapple,
    Self::SpaceJump,
    Self::ScrewAttack,
    Self::Morph,
    Self::ReserveTank,
    Self::WalljumpBoots,
    Self::DoorMissile,
    Self::DoorSuper,
    Self::DoorPowerBomb,
    Self::DoorSpazer,
    Self::DoorWave,
    Self::DoorIce,
    Self::DoorPlasma,
    Self::DoorCharge];

    pub fn to_string(self) -> String {
        match self {
            Placeable::Helm => "Starting Position",
            Placeable::ETank => "Energy Tank",
            Placeable::Missile => "Missile",
            Placeable::SuperMissile => "Super Missile",
            Placeable::PowerBomb => "Power Bomb",
            Placeable::Bombs => "Bombs",
            Placeable::Charge => "Charge",
            Placeable::Ice => "Ice",
            Placeable::HighJump => "High Jump Boots",
            Placeable::SpeedBooster => "Speed Booster",
            Placeable::Wave => "Wave",
            Placeable::Spazer => "Spazer",
            Placeable::Springball => "Springball",
            Placeable::Varia => "Varia",
            Placeable::Gravity => "Gravity",
            Placeable::XRay => "XRay",
            Placeable::Plasma => "Plasma",
            Placeable::Grapple => "Grapple",
            Placeable::SpaceJump => "Space Jump",
            Placeable::ScrewAttack => "Screw Attack",
            Placeable::Morph => "Morph",
            Placeable::ReserveTank => "Reserve Tank",
            Placeable::WalljumpBoots => "Walljump Boots",
            Placeable::DoorMissile => "Missile Door",
            Placeable::DoorSuper => "Super Door",
            Placeable::DoorPowerBomb => "Power Bomb Door",
            Placeable::DoorSpazer => "Spazer Door",
            Placeable::DoorWave => "Wave Door",
            Placeable::DoorIce => "Ice Door",
            Placeable::DoorPlasma => "Plasma Door",
            Placeable::DoorCharge => "Charge Door",
        }.to_string()
    }

    pub fn to_item(self) -> Option<Item> {
        if self == Placeable::Helm || self > Placeable::ReserveTank {
            return None;
        }
        Some(ITEM_VALUES[self as usize - Placeable::ETank as usize])
    }
}

pub struct TileInfo {
    pub room_id: usize,
    pub room_idx: usize,
    pub tile_x: usize,
    pub tile_y: usize
}

pub struct Plando {
    pub game_data: GameData,
    pub maps_vanilla: MapRepository,
    pub maps_standard: MapRepository,
    pub maps_wild: MapRepository,
    pub map: Map,
    pub randomizer_settings: RandomizerSettings,
    pub item_locations: Vec<ItemPlacement>,
    pub start_location: StartLocation,
    pub placed_item_count: [u32; Placeable::VALUES.len()],
    pub randomizable_doors: HashSet<DoorPtrPair>
}

pub enum MapRepositoryType {
    Vanilla, Standard, Wild
}

impl Plando {
    pub fn new() -> Self {
        let game_data = load_game_data().unwrap();

        let vanilla_map_path = Path::new("../maps/vanilla");
        let standard_maps_path = Path::new("../maps/v117c-standard");
        let wild_maps_path = Path::new("../maps/v117c-wild");
    
        let maps_vanilla = MapRepository::new("Vanilla", vanilla_map_path).unwrap();
        let maps_standard = MapRepository::new("Standard", standard_maps_path).unwrap();
        let maps_wild = MapRepository::new("Wild", wild_maps_path).unwrap();

        let map = roll_map(&maps_vanilla, &game_data).unwrap();
        let preset_path = Path::new("./data/presets/full-settings/Community Race Season 3 (No animals).json");
        let randomizer_settings = load_preset(preset_path).unwrap();

        let mut ship_start = StartLocation::default();
        ship_start.name = "Ship".to_string();
        ship_start.room_id = 8;
        ship_start.node_id = 5;
        ship_start.door_load_node_id = Some(2);
        ship_start.x = 72.0;
        ship_start.y = 69.5;

        let mut placed_item_count = [0u32; Placeable::VALUES.len()];
        placed_item_count[0] = 1;

        let randomizable_doors = HashSet::new();

        let mut plando = Plando {
            game_data,
            maps_vanilla,
            maps_standard,
            maps_wild,
            map,
            randomizer_settings,
            item_locations: Vec::new(),
            start_location: ship_start,
            placed_item_count,
            randomizable_doors
        };

        plando.init_item_locations();

        plando
    }

    pub fn clear_item_locations(&mut self) {
        for i in 0..self.item_locations.len() {
            self.item_locations[i].item = Item::Nothing;
        }
    }

    fn init_item_locations(&mut self) {
        for map_tile in &self.game_data.map_tile_data {
            let room_ptr = &self.game_data.room_ptr_by_id[&map_tile.room_id];
            let room_idx = self.game_data.room_idx_by_ptr[room_ptr];
            let room_geometry = &self.game_data.room_geometry[room_idx];

            for tile in &room_geometry.items {
                let mut has_double_item = false;
                for item in &mut self.item_locations {
                    if item.room_idx == room_idx && item.tile_x == tile.x && item.tile_y == tile.y {
                        has_double_item = true;
                        item.double_item_placement = DoubleItemPlacement::Left;
                    }
                }

                self.item_locations.push(ItemPlacement {
                    item: Item::Nothing,
                    room_idx,
                    tile_x: tile.x,
                    tile_y: tile.y,
                    double_item_placement: if has_double_item { DoubleItemPlacement::Right } else { DoubleItemPlacement::Middle }
                });
            }
        }
    }

    pub fn reroll_map(&mut self, map_repository: MapRepositoryType) -> Result<()> {
        let map_repository = match map_repository {
            MapRepositoryType::Vanilla => &self.maps_vanilla,
            MapRepositoryType::Standard => &self.maps_standard,
            MapRepositoryType::Wild => &self.maps_wild
        };
        self.map = roll_map(&map_repository, &self.game_data)?;
        self.clear_item_locations();
        Ok(())
    }

    pub fn load_preset(&mut self, path: &Path) -> Result<()> {
        self.randomizer_settings = load_preset(path).unwrap();
        Ok(())
    }

    pub fn get_tile_at(&self, x: usize, y: usize) -> Option<TileInfo> {
        for map_tile in &self.game_data.map_tile_data {
            let room_id = map_tile.room_id;
            let room_ptr = self.game_data.room_ptr_by_id[&room_id];
            let room_idx = self.game_data.room_idx_by_ptr[&room_ptr];
            let room_geometry = &self.game_data.room_geometry[room_idx];
            let (room_x, room_y) = self.map.rooms[room_idx];
            for (tile_y, row) in room_geometry.map.iter().enumerate() {
                for (tile_x, &tile) in row.iter().enumerate() {
                    if tile == 1 && room_x + tile_x == x && room_y + tile_y == y {
                        return Some(TileInfo { room_id, room_idx, tile_x, tile_y })
                    }
                }
            }
        }
        None
    }

    pub fn place_item(&mut self, tile_info: &TileInfo, item: Item, right_item: bool) -> Result<()> {
        for i in 0..self.item_locations.len() {
            let item_location = &mut self.item_locations[i];
            if tile_info.room_idx == item_location.room_idx && tile_info.tile_x == item_location.tile_x && tile_info.tile_y == item_location.tile_y {
                let valid = match item_location.double_item_placement {
                    DoubleItemPlacement::Middle => true,
                    DoubleItemPlacement::Left => !right_item,
                    DoubleItemPlacement::Right => right_item,
                };
                if valid {
                    // Remove old item from placed_item_count
                    if item_location.item != Item::Nothing {
                        self.placed_item_count[Placeable::ETank as usize + item_location.item as usize] -= 1;
                    }
                    // Add new item to placed_item_count
                    if item != Item::Nothing {
                        self.placed_item_count[Placeable::ETank as usize + item as usize] += 1;
                    }
                    item_location.item = item;
                    return Ok(());
                }
            }
        }
        Err(anyhow!("Could not place item"))
    }

    pub fn get_max_placeable_count(&self, placeable: Placeable) -> Option<usize> {
        if placeable == Placeable::Helm {
            return Some(1);
        } else if placeable >= Placeable::Bombs && placeable <= Placeable::Morph {
            return Some(1);
        } else if placeable == Placeable::WalljumpBoots {
            return if self.randomizer_settings.other_settings.wall_jump == WallJump::Vanilla { Some(0) } else { Some(1) };
        } else if placeable < Placeable::DoorMissile {
            let item_pool = &self.randomizer_settings.item_progression_settings.item_pool;
            return Some(match placeable {
                Placeable::Missile => item_pool.iter().find(|elem| elem.item == Item::Missile).unwrap().count,
                Placeable::SuperMissile => item_pool.iter().find(|elem| elem.item == Item::Super).unwrap().count,
                Placeable::PowerBomb => item_pool.iter().find(|elem| elem.item == Item::PowerBomb).unwrap().count,
                Placeable::ETank => item_pool.iter().find(|elem| elem.item == Item::ETank).unwrap().count,
                Placeable::ReserveTank => item_pool.iter().find(|elem| elem.item == Item::ReserveTank).unwrap().count,
                _ => 0
            });
        }
        None
    }
}

fn load_game_data() -> Result<GameData> {
    let sm_json_data_path = Path::new("../sm-json-data");
    let room_geometry_path = Path::new("../room_geometry.json");
    let escape_timings_path = Path::new("data/escape_timings.json");
    let start_locations_path = Path::new("data/start_locations.json");
    let hub_locations_path = Path::new("data/hub_locations.json");
    let title_screen_path = Path::new("../TitleScreen/Images");
    let reduced_flashing_path = Path::new("data/reduced_flashing.json");
    let strat_videos_path = Path::new("data/strat_videos.json");
    let map_tiles_path = Path::new("data/map_tiles.json");

    let game_data = GameData::load(
        sm_json_data_path,
        room_geometry_path,
        escape_timings_path,
        start_locations_path,
        hub_locations_path,
        title_screen_path,
        reduced_flashing_path,
        strat_videos_path,
        map_tiles_path,
    );

    game_data
}

fn load_preset(path: &Path) -> Result<RandomizerSettings> {
    let json_data = std::fs::read_to_string(path)?;
    let result = maprando::settings::parse_randomizer_settings(&json_data);
    result
}

fn roll_map(repo: &MapRepository, game_data: &GameData) -> Result<Map> {
    let mut rng = rand::rngs::StdRng::from_entropy();

    let map_seed = (rng.next_u64() & 0xFFFFFFFF) as usize;
    repo.get_map(1, map_seed, game_data)
}