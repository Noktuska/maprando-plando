use std::path::Path;

use anyhow::{anyhow, bail, Result};
use hashbrown::{HashMap, HashSet};
use maprando::{customize::{mosaic::MosaicTheme, samus_sprite::SamusSpriteCategory}, map_repository::MapRepository, preset::PresetData, randomize::{DebugData, DifficultyConfig, DoorState, FlagLocationState, ItemLocationState, LockedDoor, Randomization, RandomizationState, Randomizer, SaveLocationState, SpoilerDetails, SpoilerDoorDetails, SpoilerDoorSummary, SpoilerFlagDetails, SpoilerFlagSummary, SpoilerItemDetails, SpoilerItemSummary, SpoilerLocation, SpoilerLog, SpoilerSummary, StartLocationData}, settings::{Objective, RandomizerSettings, WallJump}, traverse::{apply_requirement, get_bireachable_idxs, get_spoiler_route, traverse, LockedDoorData}};
use maprando_game::{BeamType, DoorPtrPair, DoorType, GameData, HubLocation, Item, ItemLocationId, LinksDataGroup, Map, NodeId, RoomId, StartLocation, VertexKey};
use maprando_logic::{GlobalState, Inventory, LocalState};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use strum::VariantNames;

//use crate::plando_logic::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DoubleItemPlacement {
    Middle, Left, Right
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

    pub fn from_item(item: Item) -> Placeable {
        let idx = Placeable::ETank as usize + item as usize;
        Placeable::VALUES[idx]
    }

    pub fn to_item(self) -> Option<Item> {
        if self == Placeable::Helm || self > Placeable::WalljumpBoots {
            return None;
        }
        Some(ITEM_VALUES[self as usize - Placeable::ETank as usize])
    }

    pub fn to_door_type(self) -> Option<DoorType> {
        match self {
            Placeable::DoorMissile => Some(DoorType::Red),
            Placeable::DoorSuper => Some(DoorType::Green),
            Placeable::DoorPowerBomb => Some(DoorType::Yellow),
            Placeable::DoorSpazer => Some(DoorType::Beam(BeamType::Spazer)),
            Placeable::DoorWave => Some(DoorType::Beam(BeamType::Wave)),
            Placeable::DoorIce => Some(DoorType::Beam(BeamType::Ice)),
            Placeable::DoorPlasma => Some(DoorType::Beam(BeamType::Plasma)),
            Placeable::DoorCharge => Some(DoorType::Beam(BeamType::Charge)),
            _ => None
        }
    }
}

pub struct TileInfo {
    pub room_id: usize,
    pub tile_x: usize,
    pub tile_y: usize
}

pub enum MapRepositoryType {
    Vanilla, Standard, Wild
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SpoilerOverride {
    pub step: usize,
    pub item_idx: usize,
    pub description: String
}

pub struct Plando {
    pub game_data: GameData,
    pub preset_data: PresetData,
    pub samus_sprite_categories: Vec<SamusSpriteCategory>,
    pub mosaic_themes: Vec<MosaicTheme>,
    pub difficulty_tiers: Vec<DifficultyConfig>,
    pub maps_vanilla: MapRepository,
    pub maps_standard: Option<MapRepository>,
    pub maps_wild: Option<MapRepository>,
    pub map: Map,
    pub randomizer_settings: RandomizerSettings,
    pub objectives: Vec<Objective>,
    pub item_locations: Vec<Item>,
    pub start_location_data: StartLocationData,
    pub placed_item_count: [usize; Placeable::VALUES.len()],
    pub randomizable_door_connections: Vec<(DoorPtrPair, DoorPtrPair)>,
    pub locked_doors: Vec<LockedDoor>,
    pub gray_doors: HashSet<DoorPtrPair>,
    pub spoiler_overrides: Vec<SpoilerOverride>,

    door_lock_loc: Vec<(usize, usize, usize)>,
    door_beam_loc: Vec<(usize, usize, usize)>,
    total_door_count: usize,

    pub randomization: Option<(Randomization, SpoilerLog)>,

    pub rng: StdRng,
    pub auto_update_spoiler: bool,
    pub dirty: bool
}

impl Plando {
    pub fn new() -> Self {
        let game_data = load_game_data().unwrap();

        let samus_sprites_path = Path::new("../MapRandoSprites/samus_sprites/manifest.json");
        let samus_sprite_categories: Vec<SamusSpriteCategory> = serde_json::from_str(&std::fs::read_to_string(&samus_sprites_path).unwrap()).unwrap();
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
        ]
        .into_iter()
        .map(|(x, y)| MosaicTheme {
            name: x.to_string(),
            display_name: y.to_string(),
        })
        .collect();
    
        let maps_vanilla = Plando::load_map_repository(MapRepositoryType::Vanilla).expect("Vanilla Map Repository not found");
        let maps_standard = Plando::load_map_repository(MapRepositoryType::Standard);
        let maps_wild = Plando::load_map_repository(MapRepositoryType::Wild);

        if maps_standard.is_none() {
            println!("WARN: Standard Map Repository not found");
        }
        if maps_wild.is_none() {
            println!("WARN: Wild Map Repository not found");
        }

        let preset_data = load_preset_data(&game_data).unwrap();

        let map = roll_map(&maps_vanilla, &game_data).unwrap();

        let randomizer_settings = preset_data.default_preset.clone();

        let mut rng = rand::rngs::StdRng::from_entropy();
        let objectives = maprando::randomize::get_objectives(&randomizer_settings, &mut rng);
        let randomizable_door_connections = get_randomizable_door_connections(&game_data, &map, &objectives);

        let mut ship_start = StartLocation::default();
        ship_start.name = "Ship".to_string();
        ship_start.room_id = 8;
        ship_start.node_id = 5;
        ship_start.door_load_node_id = Some(2);
        ship_start.x = 72.0;
        ship_start.y = 69.5;

        let mut ship_hub = HubLocation::default();
        ship_hub.name = "Ship".to_string();
        ship_hub.room_id = 8;
        ship_hub.node_id = 5;

        let start_location_data = StartLocationData {
            start_location: ship_start,
            hub_location: ship_hub,
            hub_obtain_route: Vec::new(),
            hub_return_route: Vec::new()
        };

        let mut placed_item_count = [0usize; Placeable::VALUES.len()];
        placed_item_count[0] = 1;


        let item_location_len = game_data.item_locations.len();

        let mut plando = Plando {
            game_data,
            preset_data,
            samus_sprite_categories,
            mosaic_themes,
            difficulty_tiers: Vec::new(),
            maps_vanilla,
            maps_standard,
            maps_wild,
            map,
            randomizer_settings,
            objectives,
            item_locations: vec![Item::Nothing; item_location_len],
            start_location_data,
            placed_item_count,
            randomizable_door_connections,
            locked_doors: Vec::new(),
            gray_doors: get_gray_doors(),
            spoiler_overrides: Vec::new(),

            door_lock_loc: Vec::new(),
            door_beam_loc: Vec::new(),
            total_door_count: 0,
            randomization: None,

            rng,
            auto_update_spoiler: true,
            dirty: false
        };
        
        plando.get_difficulty_tiers();

        plando
    }

    pub fn clear_item_locations(&mut self) {
        for i in 0..self.item_locations.len() {
            self.item_locations[i] = Item::Nothing;
        }
        for i in Placeable::ETank as usize..=Placeable::WalljumpBoots as usize {
            self.placed_item_count[i] = 0;
        }
        self.spoiler_overrides.clear();

        if self.auto_update_spoiler {
            self.update_spoiler_data();
        }
    }

    pub fn room_id_to_idx(&self, id: usize) -> usize {
        let room_ptr = self.game_data.room_ptr_by_id[&id];
        self.game_data.room_idx_by_ptr[&room_ptr]
    }

    pub fn get_door_idx(&self, room_idx: usize, tile_x: usize, tile_y: usize, direction: String) -> Option<usize> {
        let door_opt = self.game_data.room_geometry[room_idx].doors.iter().position(|x| {
            x.direction == direction && x.x == tile_x && x.y == tile_y
        });
        if let Some(door_idx) = door_opt {
            return Some(door_idx);
        }
        None
    }

    pub fn load_map(&mut self, map: Map) {
        let auto_update = self.auto_update_spoiler;
        self.auto_update_spoiler = false;
        self.map = map;
        self.clear_item_locations();
        self.clear_doors();
        self.start_location_data.start_location = Plando::get_ship_start();
        let _ = self.update_hub_location();
        self.randomizable_door_connections = get_randomizable_door_connections(&self.game_data, &self.map, &self.objectives);
        self.auto_update_spoiler = auto_update;
        self.update_spoiler_data();
    }

    pub fn load_map_repository(map_repo_type: MapRepositoryType) -> Option<MapRepository> {
        let vanilla_map_path = Path::new("../maps/vanilla");
        let standard_maps_path = Path::new("../maps/v117c-standard");
        let wild_maps_path = Path::new("../maps/v117c-wild");

        match map_repo_type {
            MapRepositoryType::Vanilla => MapRepository::new("Vanilla", vanilla_map_path).ok(),
            MapRepositoryType::Standard => MapRepository::new("Standard", standard_maps_path).ok(),
            MapRepositoryType::Wild => MapRepository::new("Wild", wild_maps_path).ok()
        }
    }

    pub fn reroll_map(&mut self, map_repository: MapRepositoryType) -> Result<()> {
        let map_repository = match map_repository {
            MapRepositoryType::Vanilla => &self.maps_vanilla,
            MapRepositoryType::Standard => self.maps_standard.as_ref().unwrap(),
            MapRepositoryType::Wild => self.maps_wild.as_ref().unwrap()
        };
        let auto_update = self.auto_update_spoiler;
        self.auto_update_spoiler = false;
        self.map = roll_map(&map_repository, &self.game_data)?;
        self.clear_item_locations();
        self.clear_doors();
        self.start_location_data.start_location = Plando::get_ship_start();
        self.update_hub_location()?;
        self.randomizable_door_connections = get_randomizable_door_connections(&self.game_data, &self.map, &self.objectives);
        self.auto_update_spoiler = auto_update;
        self.update_spoiler_data();
        Ok(())
    }

    pub fn load_preset_from_file(&mut self, path: &Path) -> Result<()> {
        self.randomizer_settings = load_preset(path)?;
        self.objectives = maprando::randomize::get_objectives(&self.randomizer_settings, &mut self.rng);
        self.randomizable_door_connections = get_randomizable_door_connections(&self.game_data, &self.map, &self.objectives);
        self.get_difficulty_tiers();
        if self.auto_update_spoiler {
            self.update_spoiler_data();
        }
        Ok(())
    }

    pub fn load_preset(&mut self, preset: RandomizerSettings) {
        self.randomizer_settings = preset;
        self.objectives = maprando::randomize::get_objectives(&self.randomizer_settings, &mut self.rng);
        self.randomizable_door_connections = get_randomizable_door_connections(&self.game_data, &self.map, &self.objectives);
        self.get_difficulty_tiers();
        if self.auto_update_spoiler {
            self.update_spoiler_data();
        }
    }

    pub fn get_difficulty_tiers(&mut self) {
        self.difficulty_tiers = maprando::randomize::get_difficulty_tiers(
            &self.randomizer_settings, 
            &self.preset_data.difficulty_tiers, 
            &self.game_data, 
            &self.preset_data.tech_by_difficulty["Implicit"],
            &self.preset_data.notables_by_difficulty["Implicit"]
        );
    }

    pub fn get_tile_at(&self, x: usize, y: usize) -> Option<TileInfo> {
        for map_tile in &self.game_data.map_tile_data {
            let room_id = map_tile.room_id;
            let room_idx = self.room_id_to_idx(room_id);
            let room_geometry = &self.game_data.room_geometry[room_idx];
            let (room_x, room_y) = self.map.rooms[room_idx];
            for (tile_y, row) in room_geometry.map.iter().enumerate() {
                for (tile_x, &tile) in row.iter().enumerate() {
                    if tile == 1 && room_x + tile_x == x && room_y + tile_y == y {
                        return Some(TileInfo { room_id, tile_x, tile_y })
                    }
                }
            }
        }
        None
    }

    pub fn get_ship_start() -> StartLocation {
        let mut ship_start = StartLocation::default();
        ship_start.name = "Ship".to_string();
        ship_start.room_id = 8;
        ship_start.node_id = 5;
        ship_start.door_load_node_id = Some(2);
        ship_start.x = 72.0;
        ship_start.y = 69.5;
        ship_start
    }

    pub fn place_start_location(&mut self, start_loc: StartLocation) -> Result<()> {
        let old_start_loc = self.start_location_data.start_location.clone();
        self.start_location_data.start_location = start_loc;

        if let Err(err) = self.update_hub_location() {
            self.start_location_data.start_location = old_start_loc;
            bail!(err)
        }

        self.dirty = true;

        Ok(())
    }

    pub fn update_hub_location(&mut self) -> Result<()> {
        let start_loc = &self.start_location_data.start_location;
        // Ship location
        if start_loc.room_id == 8 && start_loc.node_id == 5 && start_loc.x == 72.0 && start_loc.y == 69.5 {
            let mut ship_hub = HubLocation::default();
            ship_hub.name = "Ship".to_string();
            ship_hub.room_id = 8;
            ship_hub.node_id = 5;
            self.start_location_data.hub_location = ship_hub;
            self.start_location_data.hub_obtain_route = Vec::new();
            self.start_location_data.hub_return_route = Vec::new();
            self.dirty = true;

            if self.auto_update_spoiler {
                self.update_spoiler_data();
            }

            return Ok(());
        }

        let locked_door_data = self.get_locked_door_data();
        let implicit_tech = &self.preset_data.tech_by_difficulty["Implicit"];
        let implicit_notables = &self.preset_data.notables_by_difficulty["Implicit"];
        let difficulty = DifficultyConfig::new(
            &self.randomizer_settings.skill_assumption_settings,
            &self.game_data,
            &implicit_tech,
            &implicit_notables,
        );
        let filtered_base_links = maprando::randomize::filter_links(&self.game_data.links, &self.game_data, &difficulty);
        let base_links_data = LinksDataGroup::new(
            filtered_base_links,
            self.game_data.vertex_isv.keys.len(),
            0,
        );
        let randomizer = Randomizer::new(
            &self.map, 
            &locked_door_data, 
            self.objectives.clone(), 
            &self.randomizer_settings,
            &self.difficulty_tiers,
            &self.game_data,
            &base_links_data,
            &mut self.rng
        );

        let num_vertices = self.game_data.vertex_isv.keys.len();
        let start_vertex_id = self.game_data.vertex_isv.index_by_key[&VertexKey {
            room_id: self.start_location_data.start_location.room_id,
            node_id: self.start_location_data.start_location.node_id,
            obstacle_mask: 0,
            actions: vec![],
        }];

        let global = self.get_initial_global_state();
        let local = apply_requirement(
            &self.start_location_data.start_location.requires_parsed.as_ref().unwrap(),
            &global,
            LocalState::new(),
            false,
            &self.randomizer_settings,
            &self.difficulty_tiers[0],
            &self.game_data,
            &locked_door_data,
            &self.objectives,
        );
        if local.is_none() {
            bail!("Invalid start location");
        }
        let forward = traverse(
            &randomizer.base_links_data,
            &randomizer.seed_links_data,
            None,
            &global,
            local.unwrap(),
            num_vertices,
            start_vertex_id,
            false,
            &self.randomizer_settings,
            &self.difficulty_tiers[0],
            &self.game_data,
            &locked_door_data,
            &self.objectives,
        );
        let forward0 = traverse(
            &randomizer.base_links_data,
            &randomizer.seed_links_data,
            None,
            &global,
            LocalState::new(),
            num_vertices,
            start_vertex_id,
            false,
            &self.randomizer_settings,
            &self.difficulty_tiers[0],
            &self.game_data,
            &locked_door_data,
            &self.objectives,
        );
        let reverse = traverse(
            &randomizer.base_links_data,
            &randomizer.seed_links_data,
            None,
            &global,
            LocalState::new(),
            num_vertices,
            start_vertex_id,
            true,
            &self.randomizer_settings,
            &self.difficulty_tiers[0],
            &self.game_data,
            &locked_door_data,
            &self.objectives,
        );

        for hub in &self.game_data.hub_locations {
            let hub_vertex_id = self.game_data.vertex_isv.index_by_key[&VertexKey {
                room_id: hub.room_id,
                node_id: hub.node_id,
                obstacle_mask: 0,
                actions: vec![]
            }];
            if !forward.cost[hub_vertex_id].iter().any(|&x| x.is_finite()) {
                continue;
            }
            if let Some((forward_cost_idx, reverse_cost_idx)) = get_bireachable_idxs(&global, hub_vertex_id, &forward0, &reverse) {
                let local = apply_requirement(
                    &hub.requires_parsed.as_ref().unwrap(),
                    &global,
                    LocalState::new(),
                    false,
                    &self.randomizer_settings,
                    &self.difficulty_tiers[0],
                    &self.game_data,
                    &locked_door_data,
                    &self.objectives
                );
                if local.is_some() {
                    let hub_obtain_link_idxs = get_spoiler_route(&forward, hub_vertex_id, forward_cost_idx);
                    let hub_return_link_idxs = get_spoiler_route(&reverse, hub_vertex_id, reverse_cost_idx);

                    let hub_obtain_route = randomizer.get_spoiler_route(&global, LocalState::new(), &hub_obtain_link_idxs, &difficulty, false);
                    let hub_return_route = randomizer.get_spoiler_route(&global, LocalState::new(), &hub_return_link_idxs, &difficulty, true);
                
                    self.start_location_data.hub_location = hub.clone();
                    self.start_location_data.hub_obtain_route = hub_obtain_route;
                    self.start_location_data.hub_return_route = hub_return_route;
                    self.dirty = true;

                    if self.auto_update_spoiler {
                        self.update_spoiler_data();
                    }

                    return Ok(());
                }
            }
        }

        bail!("Could not find suitable hub location")
    }

    pub fn place_item(&mut self, item_loc: usize, item: Item) {
        // Remove old item from placed_item_count
        if self.item_locations[item_loc] != Item::Nothing {
            self.placed_item_count[Placeable::ETank as usize + self.item_locations[item_loc] as usize] -= 1;
        }
        // Add new item to placed_item_count
        if item != Item::Nothing {
            self.placed_item_count[Placeable::ETank as usize + item as usize] += 1;
        }
        self.item_locations[item_loc] = item;
        if self.auto_update_spoiler {
            self.update_spoiler_data();
        }
        self.dirty = true;
    }

    pub fn place_door(&mut self, room_idx: usize, door_idx: usize, door_type_opt: Option<DoorType>, replace: bool, ignore_hub: bool) -> Result<()> {
        let door = &self.game_data.room_geometry[room_idx].doors[door_idx];
        let ptr_pair = (door.exit_ptr, door.entrance_ptr);

        let door_connection = self.randomizable_door_connections.iter().find(|pair| {
            pair.0 == ptr_pair || pair.1 == ptr_pair
        }).ok_or(anyhow!("Door is not randomizable. Non-randomizable doors include Gray Doors (like bosses), Sandpits, doors on the same tile as an item and doors to Save/Map/Refill rooms"))?;

        let (room_src_idx, door_src_idx) = self.game_data.room_and_door_idxs_by_door_ptr_pair[&door_connection.0];
        let (room_dst_idx, door_dst_idx) = self.game_data.room_and_door_idxs_by_door_ptr_pair[&door_connection.1];
        let tile_src_x = self.game_data.room_geometry[room_src_idx].doors[door_src_idx].x;
        let tile_src_y = self.game_data.room_geometry[room_src_idx].doors[door_src_idx].y;
        let tile_dst_x = self.game_data.room_geometry[room_dst_idx].doors[door_dst_idx].x;
        let tile_dst_y = self.game_data.room_geometry[room_dst_idx].doors[door_dst_idx].y;

        let loc_src = (room_src_idx, tile_src_x, tile_src_y);
        let loc_dst = (room_dst_idx, tile_dst_x, tile_dst_y);

        if door_type_opt.is_none() {
            if self.door_lock_loc.contains(&loc_src) && self.door_lock_loc.contains(&loc_dst) {
                self.door_lock_loc.retain(|&elem| elem != loc_src && elem != loc_dst);
            }
            if self.door_beam_loc.contains(&loc_src) && self.door_beam_loc.contains(&loc_dst) {
                self.door_beam_loc.retain(|&elem| elem != loc_src && elem != loc_dst);
            }

            let old_door_type_opt = self.locked_doors.iter().find_map(
                |elem| if elem.src_ptr_pair == door_connection.0 && elem.dst_ptr_pair == door_connection.1 { Some(elem.door_type) } else { None });

            if let Some(old_door_type) = old_door_type_opt {
                let placeable = match old_door_type {
                    DoorType::Red => Placeable::DoorMissile,
                    DoorType::Green => Placeable::DoorSuper,
                    DoorType::Yellow => Placeable::DoorPowerBomb,
                    DoorType::Beam(beam_type) => match beam_type {
                        maprando_game::BeamType::Charge => Placeable::DoorCharge,
                        maprando_game::BeamType::Ice => Placeable::DoorIce,
                        maprando_game::BeamType::Wave => Placeable::DoorWave,
                        maprando_game::BeamType::Spazer => Placeable::DoorSpazer,
                        maprando_game::BeamType::Plasma => Placeable::DoorPlasma,
                    },
                    _ => Placeable::DoorMissile
                } as usize;
                self.placed_item_count[placeable] -= 1;
                self.total_door_count -= 1;
                self.locked_doors.retain(|elem| elem.src_ptr_pair != door_connection.0 || elem.dst_ptr_pair != door_connection.1);
                
                // This should never error
                let _ = self.update_hub_location();
                if self.auto_update_spoiler {
                    self.update_spoiler_data();
                }
                self.dirty = true;
            }

            return Ok(());
        }

        if self.total_door_count == 55 {
            bail!("Cannot place more than 55 doors");
        }

        let door_type = door_type_opt.unwrap();
        let locked_door = LockedDoor {
            src_ptr_pair: door_connection.0,
            dst_ptr_pair: door_connection.1,
            door_type: door_type,
            bidirectional: true
        };

        let placeable = match door_type {
            DoorType::Red => Placeable::DoorMissile,
            DoorType::Green => Placeable::DoorSuper,
            DoorType::Yellow => Placeable::DoorPowerBomb,
            DoorType::Beam(beam_type) => match beam_type {
                maprando_game::BeamType::Charge => Placeable::DoorCharge,
                maprando_game::BeamType::Ice => Placeable::DoorIce,
                maprando_game::BeamType::Wave => Placeable::DoorWave,
                maprando_game::BeamType::Spazer => Placeable::DoorSpazer,
                maprando_game::BeamType::Plasma => Placeable::DoorPlasma,
            },
            _ => Placeable::DoorMissile
        } as usize;

        if self.door_lock_loc.contains(&loc_src) || self.door_lock_loc.contains(&loc_dst) {
            if replace {
                todo!("Implement door replacing");
            } else {
                bail!("There can only be one locked door per tile");
            }
        }

        if let DoorType::Beam(_) = door_type {
            if self.door_beam_loc.iter().any(|x| x.0 == loc_src.0 || x.0 == loc_dst.0) {
                if replace {
                    todo!("Implement door replacing");
                } else {
                    bail!("There can only be one beam door per room");
                }
            }
            self.door_beam_loc.push(loc_src);
            self.door_beam_loc.push(loc_dst);
        }
        self.door_lock_loc.push(loc_src);
        self.door_lock_loc.push(loc_dst);

        self.locked_doors.push(locked_door);
        
        self.placed_item_count[placeable] += 1;
        self.total_door_count += 1;

        // Door blocks current and all hub locations
        if !ignore_hub && self.update_hub_location().is_err() {
            // Undo placement
            let _ = self.place_door(room_idx, door_idx, None, true, true);
            bail!("Placing door would block all logical hub locations");
        }
        if self.auto_update_spoiler {
            self.update_spoiler_data();
        }
        self.dirty = true;

        Ok(())
    }

    pub fn clear_doors(&mut self) {
        self.door_beam_loc.clear();
        self.door_lock_loc.clear();
        self.locked_doors.clear();
        self.total_door_count = 0;

        for i in Placeable::DoorMissile as usize..Placeable::DoorPlasma as usize {
            self.placed_item_count[i] = 0;
        }

        if self.auto_update_spoiler {
            self.update_spoiler_data();
        }
    }

    pub fn get_max_placeable_count(&self, placeable: Placeable) -> Option<usize> {
        if placeable == Placeable::Helm {
            return Some(1);
        } else if placeable >= Placeable::Bombs && placeable <= Placeable::Morph {
            let item = placeable.to_item().unwrap();
            if let Some(item_count) = self.randomizer_settings.item_progression_settings.starting_items.iter().find(|x| x.item == item) {
                return Some(1 - item_count.count);
            }
            return Some(1);
        } else if placeable == Placeable::WalljumpBoots {
            return if self.randomizer_settings.other_settings.wall_jump == WallJump::Vanilla { Some(0) } else { Some(1) };
        } else if placeable < Placeable::DoorMissile {
            let item_pool = &self.randomizer_settings.item_progression_settings.item_pool;
            let count = match placeable {
                Placeable::Missile => item_pool.iter().find(|elem| elem.item == Item::Missile).unwrap().count,
                Placeable::SuperMissile => item_pool.iter().find(|elem| elem.item == Item::Super).unwrap().count,
                Placeable::PowerBomb => item_pool.iter().find(|elem| elem.item == Item::PowerBomb).unwrap().count,
                Placeable::ETank => item_pool.iter().find(|elem| elem.item == Item::ETank).unwrap().count,
                Placeable::ReserveTank => item_pool.iter().find(|elem| elem.item == Item::ReserveTank).unwrap().count,
                _ => 0
            };
            let item = placeable.to_item().unwrap();
            if let Some(item_count) = self.randomizer_settings.item_progression_settings.starting_items.iter().find(|x| x.item == item) {
                return Some(count - item_count.count);
            }
            return Some(count);
        }
        None
    }

    pub fn get_locked_door_data(&self) -> LockedDoorData {
        let mut locked_door_node_map: HashMap<(RoomId, NodeId), usize> = HashMap::new();
        for (i, door) in self.locked_doors.iter().enumerate() {
            let (src_room_id, src_node_id) = self.game_data.door_ptr_pair_map[&door.src_ptr_pair];
            locked_door_node_map.insert((src_room_id, src_node_id), i);
            if door.bidirectional {
                let (dst_room_id, dst_node_id) = self.game_data.door_ptr_pair_map[&door.dst_ptr_pair];
                locked_door_node_map.insert((dst_room_id, dst_node_id), i);
            }
        }

        // Homing Geemer Room left door -> West Ocean Bridge left door
        if let Some(&idx) = locked_door_node_map.get(&(313, 1)) {
            locked_door_node_map.insert((32, 7), idx);
        }

        // Homing Geemer Room right door -> West Ocean Bridge right door
        if let Some(&idx) = locked_door_node_map.get(&(313, 2)) {
            locked_door_node_map.insert((32, 8), idx);
        }

        // Pants Room right door -> East Pants Room right door
        if let Some(&idx) = locked_door_node_map.get(&(322, 2)) {
            locked_door_node_map.insert((220, 2), idx);
        }

        let mut locked_door_vertex_ids = vec![vec![]; self.locked_doors.len()];
        for (&(room_id, node_id), vertex_ids) in &self.game_data.node_door_unlock {
            if let Some(&locked_door_idx) = locked_door_node_map.get(&(room_id, node_id)) {
                locked_door_vertex_ids[locked_door_idx].extend(vertex_ids);
            }
        }

        LockedDoorData {
            locked_doors: self.locked_doors.clone(),
            locked_door_node_map,
            locked_door_vertex_ids,
        }
    }

    pub fn get_vertex_info(&self, vertex_id: usize) -> VertexInfo {
        let VertexKey {
            room_id, node_id, ..
        } = self.game_data.vertex_isv.keys[vertex_id];
        self.get_vertex_info_by_id(room_id, node_id)
    }

    pub fn get_vertex_info_by_id(&self, room_id: RoomId, node_id: NodeId) -> VertexInfo {
        let room_ptr = self.game_data.room_ptr_by_id[&room_id];
        let room_idx = self.game_data.room_idx_by_ptr[&room_ptr];
        let area = self.map.area[room_idx];
        let room_coords = self.map.rooms[room_idx];
        VertexInfo {
            area_name: self.game_data.area_names[area].clone(),
            room_name: self.game_data.room_json_map[&room_id]["name"]
                .as_str()
                .unwrap()
                .to_string(),
            room_id,
            room_coords,
            node_name: self.game_data.node_json_map[&(room_id, node_id)]["name"]
                .as_str()
                .unwrap()
                .to_string(),
            node_id,
        }
    }

    pub fn update_spoiler_data(&mut self) {
        let locked_door_data = self.get_locked_door_data();
        let implicit_tech = &self.preset_data.tech_by_difficulty["Implicit"];
        let implicit_notables = &self.preset_data.notables_by_difficulty["Implicit"];
        let difficulty = DifficultyConfig::new(
            &self.randomizer_settings.skill_assumption_settings,
            &self.game_data,
            &implicit_tech,
            &implicit_notables,
        );
        let filtered_base_links = maprando::randomize::filter_links(&self.game_data.links, &self.game_data, &difficulty);
        let base_links_data = LinksDataGroup::new(
            filtered_base_links,
            self.game_data.vertex_isv.keys.len(),
            0,
        );
        let randomizer = Randomizer::new(
            &self.map, 
            &locked_door_data, 
            self.objectives.clone(), 
            &self.randomizer_settings,
            &self.difficulty_tiers,
            &self.game_data,
            &base_links_data,
            &mut self.rng
        );

        let initial_global_state = self.get_initial_global_state();
        let initial_item_location_state = ItemLocationState {
            placed_item: None,
            collected: false,
            reachable_step: None,
            bireachable: false,
            bireachable_vertex_id: None,
            difficulty_tier: None,
        };
        let initial_flag_location_state = FlagLocationState {
            reachable_step: None,
            reachable_vertex_id: None,
            bireachable: false,
            bireachable_vertex_id: None,
        };
        let initial_save_location_state = SaveLocationState { bireachable: false };
        let initial_door_state = DoorState {
            bireachable: false,
            bireachable_vertex_id: None,
        };

        let mut state = RandomizationState {
            step_num: 1,
            start_location: self.start_location_data.start_location.clone(),
            hub_location: self.start_location_data.hub_location.clone(),
            hub_obtain_route: self.start_location_data.hub_obtain_route.clone(),
            hub_return_route: self.start_location_data.hub_return_route.clone(),
            item_precedence: Vec::new(),
            save_location_state: vec![initial_save_location_state; self.game_data.save_locations.len()],
            item_location_state: vec![initial_item_location_state; self.game_data.item_locations.len()],
            flag_location_state: vec![initial_flag_location_state; self.game_data.flag_ids.len()],
            door_state: vec![initial_door_state; locked_door_data.locked_doors.len()],
            items_remaining: randomizer.initial_items_remaining.clone(),
            global_state: initial_global_state,
            debug_data: None,
            previous_debug_data: None,
            key_visited_vertices: HashSet::new()
        };

        randomizer.update_reachability(&mut state);

        for i in 0..state.item_location_state.len() {
            if self.item_locations[i] != Item::Nothing {
                state.item_location_state[i].placed_item = Some(self.item_locations[i]);
            }
        }

        let mut spoiler_summary_vec = vec![];
        let mut spoiler_details_vec = vec![];
        let mut debug_data_vec: Vec<DebugData> = Vec::new();

        loop {
            let (spoiler_summary, spoiler_details) = self.update_step(&mut state, &randomizer);
            let any_progress = spoiler_summary.items.len() > 0 || spoiler_summary.flags.len() > 0;
            spoiler_summary_vec.push(spoiler_summary);
            spoiler_details_vec.push(spoiler_details);
            debug_data_vec.push(state.previous_debug_data.as_ref().unwrap().clone());

            if !any_progress {
                break;
            }
        }

        for item_loc_state in &mut state.item_location_state {
            if item_loc_state.placed_item.is_none() {
                item_loc_state.placed_item = Some(Item::Nothing);
            }
        }

        let seed_part = (self.rng.next_u32() % 0xFE) + 1; // Generate seed_part 1-255 so seed can't be 0
        let seed = seed_part | (seed_part << 8) | (seed_part << 16) | (seed_part << 24);

        self.randomization = randomizer.get_randomization(
            &state,
            spoiler_summary_vec,
            spoiler_details_vec,
            debug_data_vec,
            seed as usize,
            seed as usize,
            &mut self.rng
        ).ok();
    }

    fn update_step(&self, state: &mut RandomizationState, randomizer: &Randomizer) -> (SpoilerSummary, SpoilerDetails) {
        let orig_global_state = state.global_state.clone();
        let mut spoiler_flag_summaries: Vec<SpoilerFlagSummary> = Vec::new();
        let mut spoiler_flag_details: Vec<SpoilerFlagDetails> = Vec::new();
        let mut spoiler_door_summaries: Vec<SpoilerDoorSummary> = Vec::new();
        let mut spoiler_door_details: Vec<SpoilerDoorDetails> = Vec::new();
        loop {
            let mut any_update = false;
            for (i, &flag_id) in self.game_data.flag_ids.iter().enumerate() {
                if state.global_state.flags[flag_id] {
                    continue;
                }
                if state.flag_location_state[i].reachable_step.is_some() && flag_id == self.game_data.mother_brain_defeated_flag_id {
                    any_update = true;
                    let flag_vertex_id = state.flag_location_state[i].reachable_vertex_id.unwrap();
                    spoiler_flag_summaries.push(randomizer.get_spoiler_flag_summary(&state, flag_vertex_id, flag_id));
                    spoiler_flag_details.push(randomizer.get_spoiler_flag_details_one_way(&state, flag_vertex_id, flag_id, i));
                    state.global_state.flags[flag_id] = true;
                } else if state.flag_location_state[i].bireachable {
                    any_update = true;
                    let flag_vertex_id = state.flag_location_state[i].bireachable_vertex_id.unwrap();
                    spoiler_flag_summaries.push(randomizer.get_spoiler_flag_summary(&state, flag_vertex_id, flag_id));
                    spoiler_flag_details.push(randomizer.get_spoiler_flag_details(&state, flag_vertex_id, flag_id, i));
                    state.global_state.flags[flag_id] = true;
                }
            }
            for i in 0..randomizer.locked_door_data.locked_doors.len() {
                if state.global_state.doors_unlocked[i] {
                    continue;
                }
                if state.door_state[i].bireachable {
                    any_update = true;
                    let door_vertex_id = state.door_state[i].bireachable_vertex_id.unwrap();
                    spoiler_door_summaries.push(randomizer.get_spoiler_door_summary(door_vertex_id, i));
                    spoiler_door_details.push(randomizer.get_spoiler_door_details(&state, door_vertex_id, i));
                    state.global_state.doors_unlocked[i] = true;
                }
            }
            if any_update {
                randomizer.update_reachability(state);
            } else {
                break;
            }
        }

        let mut placed_uncollected_bireachable_loc: Vec<ItemLocationId> = Vec::new();
        let mut placed_uncollected_bireachable_items: Vec<Item> = Vec::new();
        for (i, item_location_state) in state.item_location_state.iter().enumerate() {
            if let Some(item) = item_location_state.placed_item {
                if !item_location_state.collected && item_location_state.bireachable {
                    placed_uncollected_bireachable_loc.push(i);
                    placed_uncollected_bireachable_items.push(item);
                }
            }
        }

        let mut new_state = RandomizationState {
            step_num: state.step_num + 1,
            start_location: state.start_location.clone(),
            hub_location: state.hub_location.clone(),
            hub_obtain_route: state.hub_obtain_route.clone(),
            hub_return_route: state.hub_return_route.clone(),
            item_precedence: state.item_precedence.clone(),
            item_location_state: state.item_location_state.clone(),
            flag_location_state: state.flag_location_state.clone(),
            save_location_state: state.save_location_state.clone(),
            door_state: state.door_state.clone(),
            items_remaining: state.items_remaining.clone(),
            global_state: state.global_state.clone(),
            debug_data: None,
            previous_debug_data: None,
            key_visited_vertices: HashSet::new(),
        };
        new_state.previous_debug_data = state.debug_data.clone();

        for &item in &placed_uncollected_bireachable_items {
            new_state.global_state.collect(item, &self.game_data, self.randomizer_settings.item_progression_settings.ammo_collect_fraction, &self.difficulty_tiers[0].tech);
        }
        // Add overrides to the current step
        let overrides: Vec<_> = self.spoiler_overrides.iter().filter(|x| x.step == state.step_num).collect();
        for item_override in &overrides {
            let item = self.item_locations[item_override.item_idx];
            new_state.global_state.collect(item, &self.game_data, self.randomizer_settings.item_progression_settings.ammo_collect_fraction, &self.difficulty_tiers[0].tech);
        }

        randomizer.update_reachability(&mut new_state);

        for &loc in &placed_uncollected_bireachable_loc {
            new_state.item_location_state[loc].collected = true;
        }

        let mut spoiler_summary = randomizer.get_spoiler_summary(
            &orig_global_state,
            &state,
            &new_state,
            spoiler_flag_summaries,
            spoiler_door_summaries
        );
        let mut spoiler_details = randomizer.get_spoiler_details(
            &orig_global_state,
            &state,
            &new_state,
            spoiler_flag_details,
            spoiler_door_details
        );

        // Mark items as collected after getting spoiler data as they are not logicall bireachable
        for item_override in overrides {
            let state = &mut new_state.item_location_state[item_override.item_idx];
            if state.collected {
                continue;
            }
            state.collected = true;
            state.bireachable = true;
            state.bireachable_vertex_id = self.game_data.item_vertex_ids[item_override.item_idx].first().copied();
            state.reachable_step = Some(new_state.step_num);

            let item = self.item_locations[item_override.item_idx];
            let item_str: String = Item::VARIANTS[item as usize].to_string();
            let (room_id, node_id) = self.game_data.item_locations[item_override.item_idx];
            let vertex_info = self.get_vertex_info_by_id(room_id, node_id);

            // Dummy fill spoiler summary and details
            spoiler_summary.items.push(SpoilerItemSummary {
                item: item_str.clone(),
                location: SpoilerLocation {
                    area: vertex_info.area_name.clone(),
                    room_id: vertex_info.room_id,
                    room: vertex_info.room_name.clone(),
                    node_id: vertex_info.node_id,
                    node: vertex_info.node_name.clone(),
                    coords: vertex_info.room_coords
                }
            });
            spoiler_details.items.push(SpoilerItemDetails {
                item: item_str,
                location: SpoilerLocation {
                    area: vertex_info.area_name,
                    room_id: vertex_info.room_id,
                    room: vertex_info.room_name,
                    node_id: vertex_info.node_id,
                    node: vertex_info.node_name,
                    coords: vertex_info.room_coords
                },
                reachable_step: new_state.step_num,
                difficulty: Some("Custom".to_string()),
                obtain_route: Vec::new(),
                return_route: Vec::new()
            });
        }

        *state = new_state;
        (spoiler_summary, spoiler_details)
    }

    /* COPY FROM maprando::randomize::get_initial_global_state */
    fn get_initial_global_state(&self) -> GlobalState {
        let items = vec![false; self.game_data.item_isv.keys.len()];
        let weapon_mask = self
            .game_data
            .get_weapon_mask(&items, &self.difficulty_tiers[0].tech);
        let mut global = GlobalState {
            inventory: Inventory {
                items: items,
                max_energy: 99,
                max_reserves: 0,
                max_missiles: 0,
                max_supers: 0,
                max_power_bombs: 0,
                collectible_missile_packs: 0,
                collectible_super_packs: 0,
                collectible_power_bomb_packs: 0,
            },
            flags: self.get_initial_flag_vec(),
            doors_unlocked: vec![false; self.locked_doors.len()],
            weapon_mask: weapon_mask,
        };
        for x in &self.randomizer_settings.item_progression_settings.starting_items {
            for _ in 0..x.count {
                global.collect(
                    x.item,
                    &self.game_data,
                    self.randomizer_settings
                        .item_progression_settings
                        .ammo_collect_fraction,
                    &self.difficulty_tiers[0].tech,
                );
            }
        }
        global
    }

    fn get_initial_flag_vec(&self) -> Vec<bool> {
        let mut flag_vec = vec![false; self.game_data.flag_isv.keys.len()];
        let tourian_open_idx = self.game_data.flag_isv.index_by_key["f_TourianOpen"];
        flag_vec[tourian_open_idx] = true;
        if self.randomizer_settings.quality_of_life_settings.all_items_spawn {
            let all_items_spawn_idx = self.game_data.flag_isv.index_by_key["f_AllItemsSpawn"];
            flag_vec[all_items_spawn_idx] = true;
        }
        if self.randomizer_settings.quality_of_life_settings.acid_chozo {
            let acid_chozo_without_space_jump_idx =
                self.game_data.flag_isv.index_by_key["f_AcidChozoWithoutSpaceJump"];
            flag_vec[acid_chozo_without_space_jump_idx] = true;
        }
        flag_vec
    }
}

fn load_game_data() -> Result<GameData> {
    let sm_json_data_path = Path::new("../sm-json-data");
    let room_geometry_path = Path::new("../room_geometry.json");
    let escape_timings_path = Path::new("./data/escape_timings.json");
    let start_locations_path = Path::new("./data/start_locations.json");
    let hub_locations_path = Path::new("./data/hub_locations.json");
    let title_screen_path = Path::new("../TitleScreen/Images");
    let reduced_flashing_path = Path::new("./data/reduced_flashing.json");
    let strat_videos_path = Path::new("./data/strat_videos.json");
    let map_tiles_path = Path::new("./data/map_tiles.json");

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

pub fn load_preset(path: &Path) -> Result<RandomizerSettings> {
    let json_data = std::fs::read_to_string(path)?;
    let result = maprando::settings::parse_randomizer_settings(&json_data);
    result
}

fn load_preset_data(game_data: &GameData) -> Result<PresetData> {
    let tech_path = Path::new("./data/tech_data.json");
    let notable_path = Path::new("./data/notable_data.json");
    let presets_path = Path::new("./data/presets/");

    let preset_data = PresetData::load(tech_path, notable_path, presets_path, game_data);
    preset_data
}

fn roll_map(repo: &MapRepository, game_data: &GameData) -> Result<Map> {
    let mut rng = rand::rngs::StdRng::from_entropy();

    let map_seed = (rng.next_u64() & 0xFFFFFFFF) as usize;
    repo.get_map(1, map_seed, game_data)
}



pub struct VertexInfo {
    pub area_name: String,
    pub room_id: usize,
    pub room_name: String,
    pub room_coords: (usize, usize),
    pub node_name: String,
    pub node_id: usize,
}

fn get_gray_doors() -> HashSet<DoorPtrPair> {
    let result: HashSet<DoorPtrPair> = vec![
        // Gray doors - Pirate rooms:
        (0x18B7A, 0x18B62), // Pit Room left
        (0x18B86, 0x18B92), // Pit Room right
        (0x19192, 0x1917A), // Baby Kraid left
        (0x1919E, 0x191AA), // Baby Kraid right
        (0x1A558, 0x1A54C), // Plasma Room
        (0x19A32, 0x19966), // Metal Pirates left
        (0x19A3E, 0x19A1A), // Metal Pirates right
        // Gray doors - Bosses:
        (0x191CE, 0x191B6), // Kraid left
        (0x191DA, 0x19252), // Kraid right
        (0x1A2C4, 0x1A2AC), // Phantoon
        (0x1A978, 0x1A924), // Draygon left
        (0x1A96C, 0x1A840), // Draygon right
        (0x198B2, 0x19A62), // Ridley left
        (0x198BE, 0x198CA), // Ridley right
        (0x1AA8C, 0x1AAE0), // Mother Brain left
        (0x1AA80, 0x1AAC8), // Mother Brain right
        // Gray doors - Minibosses:
        (0x18BAA, 0x18BC2), // Bomb Torizo
        (0x18E56, 0x18E3E), // Spore Spawn bottom
        (0x193EA, 0x193D2), // Crocomire top
        (0x1A90C, 0x1A774), // Botwoon left
        (0x19882, 0x19A86), // Golden Torizo right
    ].into_iter().map(|(l, r)| (Some(l), Some(r))).collect();

    result
}

fn get_randomizable_doors(game_data: &GameData, objectives: &[Objective]) -> HashSet<DoorPtrPair> {
    // Doors which we do not want to randomize:
    let mut non_randomizable_doors: HashSet<DoorPtrPair> = vec![
        // Gray doors - Pirate rooms:
        (0x18B7A, 0x18B62), // Pit Room left
        (0x18B86, 0x18B92), // Pit Room right
        (0x19192, 0x1917A), // Baby Kraid left
        (0x1919E, 0x191AA), // Baby Kraid right
        (0x1A558, 0x1A54C), // Plasma Room
        (0x19A32, 0x19966), // Metal Pirates left
        (0x19A3E, 0x19A1A), // Metal Pirates right
        // Gray doors - Bosses:
        (0x191CE, 0x191B6), // Kraid left
        (0x191DA, 0x19252), // Kraid right
        (0x1A2C4, 0x1A2AC), // Phantoon
        (0x1A978, 0x1A924), // Draygon left
        (0x1A96C, 0x1A840), // Draygon right
        (0x198B2, 0x19A62), // Ridley left
        (0x198BE, 0x198CA), // Ridley right
        (0x1AA8C, 0x1AAE0), // Mother Brain left
        (0x1AA80, 0x1AAC8), // Mother Brain right
        // Gray doors - Minibosses:
        (0x18BAA, 0x18BC2), // Bomb Torizo
        (0x18E56, 0x18E3E), // Spore Spawn bottom
        (0x193EA, 0x193D2), // Crocomire top
        (0x1A90C, 0x1A774), // Botwoon left
        (0x19882, 0x19A86), // Golden Torizo right
        // Save stations:
        (0x189BE, 0x1899A), // Crateria Save Room
        (0x19006, 0x18D12), // Green Brinstar Main Shaft Save Room
        (0x19012, 0x18F52), // Etecoon Save Room
        (0x18FD6, 0x18DF6), // Big Pink Save Room
        (0x1926A, 0x190D2), // Caterpillar Save Room
        (0x1925E, 0x19186), // Warehouse Save Room
        (0x1A828, 0x1A744), // Aqueduct Save Room
        (0x1A888, 0x1A7EC), // Draygon Save Room left
        (0x1A87C, 0x1A930), // Draygon Save Room right
        (0x1A5F4, 0x1A588), // Forgotten Highway Save Room
        (0x1A324, 0x1A354), // Glass Tunnel Save Room
        (0x19822, 0x193BA), // Crocomire Save Room
        (0x19462, 0x19456), // Post Crocomire Save Room
        (0x1982E, 0x19702), // Lower Norfair Elevator Save Room
        (0x19816, 0x192FA), // Frog Savestation left
        (0x1980A, 0x197DA), // Frog Savestation right
        (0x197CE, 0x1959A), // Bubble Mountain Save Room
        (0x19AB6, 0x19A0E), // Red Kihunter Shaft Save Room
        (0x1A318, 0x1A240), // Wrecked Ship Save Room
        (0x1AAD4, 0x1AABC), // Lower Tourian Save Room
        // Map stations:
        (0x18C2E, 0x18BDA), // Crateria Map Room
        (0x18D72, 0x18D36), // Brinstar Map Room
        (0x197C2, 0x19306), // Norfair Map Room
        (0x1A5E8, 0x1A51C), // Maridia Map Room
        (0x1A2B8, 0x1A2A0), // Wrecked Ship Map Room
        (0x1AB40, 0x1A99C), // Tourian Map Room (Upper Tourian Save Room)
        // Refill stations:
        (0x18D96, 0x18D7E), // Green Brinstar Missile Refill Room
        (0x18F6A, 0x18DBA), // Dachora Energy Refill Room
        (0x191FE, 0x1904E), // Sloaters Refill
        (0x1A894, 0x1A8F4), // Maridia Missile Refill Room
        (0x1A930, 0x1A87C), // Maridia Health Refill Room
        (0x19786, 0x19756), // Nutella Refill left
        (0x19792, 0x1976E), // Nutella Refill right
        (0x1920A, 0x191C2), // Kraid Recharge Station
        (0x198A6, 0x19A7A), // Golden Torizo Energy Recharge
        (0x1AA74, 0x1AA68), // Tourian Recharge Room
        // Pants room interior door
        (0x1A7A4, 0x1A78C), // Left door
        (0x1A78C, 0x1A7A4), // Right door
        // Items: (to avoid an interaction in map tiles between doors disappearing and items disappearing)
        (0x18FA6, 0x18EDA), // First Missile Room
        (0x18FFA, 0x18FEE), // Billy Mays Room
        (0x18D66, 0x18D5A), // Brinstar Reserve Tank Room
        (0x18F3A, 0x18F5E), // Etecoon Energy Tank Room (top left door)
        (0x18F5E, 0x18F3A), // Etecoon Supers Room
        (0x18E02, 0x18E62), // Big Pink (top door to Pink Brinstar Power Bomb Room)
        (0x18FCA, 0x18FBE), // Hopper Energy Tank Room
        (0x19132, 0x19126), // Spazer Room
        (0x19162, 0x1914A), // Warehouse Energy Tank Room
        (0x19252, 0x191DA), // Varia Suit Room
        (0x18ADE, 0x18A36), // The Moat (left door)
        (0x18C9A, 0x18C82), // The Final Missile
        (0x18BE6, 0x18C3A), // Terminator Room (left door)
        (0x18B0E, 0x18952), // Gauntlet Energy Tank Room (right door)
        (0x1A924, 0x1A978), // Space Jump Room
        (0x19A62, 0x198B2), // Ridley Tank Room
        (0x199D2, 0x19A9E), // Lower Norfair Escape Power Bomb Room (left door)
        (0x199DE, 0x199C6), // Lower Norfair Escape Power Bomb Room (top door)
        (0x19876, 0x1983A), // Golden Torizo's Room (left door)
        (0x19A86, 0x19882), // Screw Attack Room (left door)
        (0x1941A, 0x192D6), // Hi Jump Energy Tank Room (right door)
        (0x193F6, 0x19426), // Hi Jump Boots Room
        (0x1929A, 0x19732), // Cathedral (right door)
        (0x1953A, 0x19552), // Green Bubbles Missile Room
        (0x195B2, 0x195BE), // Speed Booster Hall
        (0x195BE, 0x195B2), // Speed Booster Room
        (0x1962A, 0x1961E), // Wave Beam Room
        (0x1935A, 0x1937E), // Ice Beam Room
        (0x1938A, 0x19336), // Crumble Shaft (top right door)
        (0x19402, 0x192E2), // Crocomire Escape (left door)
        (0x1946E, 0x1943E), // Post Crocomire Power Bomb Room
        (0x19516, 0x194DA), // Grapple Beam Room (bottom right door)
        (0x1A2E8, 0x1A210), // Wrecked Ship West Super Room
        (0x1A300, 0x18A06), // Gravity Suit Room (left door)
        (0x1A30C, 0x1A1A4), // Gravity Suit Room (right door)
    ]
    .into_iter()
    .map(|(x, y)| (Some(x), Some(y)))
    .collect();

    // Avoid placing an ammo door on a tile with an objective "X", as it looks bad.
    for i in objectives.iter() {
        use Objective::*;
        match i {
            SporeSpawn => {
                non_randomizable_doors.insert((Some(0x18E4A), Some(0x18D2A)));
            }
            Crocomire => {
                non_randomizable_doors.insert((Some(0x193DE), Some(0x19432)));
            }
            Botwoon => {
                non_randomizable_doors.insert((Some(0x1A918), Some(0x1A84C)));
            }
            GoldenTorizo => {
                non_randomizable_doors.insert((Some(0x19876), Some(0x1983A)));
            }
            MetroidRoom1 => {
                non_randomizable_doors.insert((Some(0x1A9B4), Some(0x1A9C0))); // left
                non_randomizable_doors.insert((Some(0x1A9A8), Some(0x1A984))); // right
            }
            MetroidRoom2 => {
                non_randomizable_doors.insert((Some(0x1A9C0), Some(0x1A9B4))); // top right
                non_randomizable_doors.insert((Some(0x1A9CC), Some(0x1A9D8))); // bottom right
            }
            MetroidRoom3 => {
                non_randomizable_doors.insert((Some(0x1A9D8), Some(0x1A9CC))); // left
                non_randomizable_doors.insert((Some(0x1A9E4), Some(0x1A9F0))); // right
            }
            MetroidRoom4 => {
                non_randomizable_doors.insert((Some(0x1A9F0), Some(0x1A9E4))); // left
                non_randomizable_doors.insert((Some(0x1A9FC), Some(0x1AA08))); // bottom
            }
            _ => {} // All other tiles have gray doors and are excluded above.
        }
    }

    let mut out: Vec<DoorPtrPair> = vec![];
    for room in &game_data.room_geometry {
        for door in &room.doors {
            let pair = (door.exit_ptr, door.entrance_ptr);
            let has_door_cap = door.offset.is_some();
            if has_door_cap && !non_randomizable_doors.contains(&pair) {
                out.push(pair);
            }
        }
    }
    out.into_iter().collect()
}

fn get_randomizable_door_connections(
    game_data: &GameData,
    map: &Map,
    objectives: &[Objective],
) -> Vec<(DoorPtrPair, DoorPtrPair)> {
    let doors = get_randomizable_doors(game_data, objectives);
    let mut out: Vec<(DoorPtrPair, DoorPtrPair)> = vec![];
    for (src_door_ptr_pair, dst_door_ptr_pair, _bidirectional) in &map.doors {
        if doors.contains(src_door_ptr_pair) && doors.contains(dst_door_ptr_pair) {
            out.push((*src_door_ptr_pair, *dst_door_ptr_pair));
        }
    }
    out
}

pub fn get_double_item_offset(room_id: usize, node_id: usize) -> DoubleItemPlacement {
    match room_id {
        19 => DoubleItemPlacement::Left, // Bomb Torizo
        46 => if node_id == 4 { DoubleItemPlacement::Right } else if node_id == 3 { DoubleItemPlacement::Left } else { DoubleItemPlacement::Middle }, // Brinstar Reserve
        43 => if node_id == 2 { DoubleItemPlacement::Right } else { DoubleItemPlacement::Left }, // Billy Mays
        99 => if node_id == 3 { DoubleItemPlacement::Right } else { DoubleItemPlacement::Left }, // Norfair Reserve
        181 => if node_id == 3 { DoubleItemPlacement::Right } else { DoubleItemPlacement::Left }, // Watering Hole
        209 => if node_id == 4 { DoubleItemPlacement::Right } else { DoubleItemPlacement::Left }, // West Sand Hole
        21 => if node_id == 6 { DoubleItemPlacement::Right } else { DoubleItemPlacement::Left }, // Green Pirates Shaft
        _ => DoubleItemPlacement::Middle
    }
}