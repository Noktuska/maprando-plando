use std::{path::Path, sync::{Arc, MutexGuard}};

use anyhow::{anyhow, bail, Result};
use hashbrown::{HashMap, HashSet};
use maprando::{customize::{mosaic::MosaicTheme, samus_sprite::SamusSpriteCategory, CustomizeSettings}, map_repository::MapRepository, patch::Rom, preset::PresetData, randomize::{DifficultyConfig, LockedDoor, Randomization, SpoilerLog}, settings::{DoorsMode, ItemCount, Objective, RandomizerSettings, WallJump}, traverse::LockedDoorData};
use maprando_game::{BeamType, DoorPtrPair, DoorType, GameData, HubLocation, Item, Map, NodeId, RoomId, StartLocation, VertexKey};
use maprando_logic::{GlobalState, Inventory, LocalState};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use strum::VariantArray;
use strum_macros::VariantArray;
use tokio::task::JoinHandle;

use crate::backend::{logic::{HubLocationData, Logic}, map_editor::{MapEditor, MapErrorType}, randomize::{get_gray_doors, get_randomizable_doors}};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DoubleItemPlacement {
    Middle, Left, Right
}

pub const ITEM_VALUES: [Item; 23] = [
    Item::ETank, Item::Missile, Item::Super, Item::PowerBomb, Item::Bombs, Item::Charge, Item::Ice, Item::HiJump, Item::SpeedBooster,
    Item::Wave, Item::Spazer, Item::SpringBall, Item::Varia, Item::Gravity, Item::XRayScope, Item::Plasma, Item::Grapple,
    Item::SpaceJump, Item::ScrewAttack, Item::Morph, Item::ReserveTank, Item::WallJump, Item::Nothing
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, VariantArray)]
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
    DoorSpazer,
    DoorWave,
    DoorIce,
    DoorPlasma,
    DoorCharge,
    DoorWall,
}

impl Placeable {
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
            Placeable::DoorWall => "Wall Door"
        }.to_string()
    }

    pub fn from_item(item: Item) -> Placeable {
        let idx = Placeable::ETank as usize + item as usize;
        Placeable::VARIANTS[idx]
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
            Placeable::DoorWall => Some(DoorType::Wall),
            _ => None
        }
    }

    pub fn from_door_type(door_type: DoorType) -> Option<Self> {
        match door_type {
            DoorType::Red => Some(Placeable::DoorMissile),
            DoorType::Green => Some(Placeable::DoorSuper),
            DoorType::Yellow => Some(Placeable::DoorPowerBomb),
            DoorType::Wall => Some(Placeable::DoorWall),
            DoorType::Beam(beam) => Some(match beam {
                BeamType::Charge => Placeable::DoorCharge,
                BeamType::Ice => Placeable::DoorIce,
                BeamType::Plasma => Placeable::DoorPlasma,
                BeamType::Spazer => Placeable::DoorSpazer,
                BeamType::Wave => Placeable::DoorWave,
            }),
            _ => None
        }
    }
}

pub enum MapRepositoryType {
    Vanilla, Standard, Wild
}

struct MapRepositoryWrapper {
    repo: MapRepository,
    cache: Vec<Map>
}

impl MapRepositoryWrapper {
    fn roll_map(&mut self, rng: &mut StdRng, game_data: &GameData) -> Result<Map> {
        if self.cache.is_empty() {
            let map_seed = (rng.next_u64() & 0xFFFFFFFF) as usize;
            self.cache = self.repo.get_map_batch(map_seed, game_data)?;
        }

        let map_idx = (rng.next_u64() % self.cache.len() as u64) as usize;
        Ok(self.cache[map_idx].clone())
    }
}

impl From<MapRepository> for MapRepositoryWrapper {
    fn from(value: MapRepository) -> Self {
        MapRepositoryWrapper {
            repo: value,
            cache: Vec::new()
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SpoilerOverride {
    pub step: usize,
    pub item_idx: usize,
    pub description: String
}

struct ImplicitPresetData {
    difficulty_tiers: Vec<DifficultyConfig>,
    implicit_tech: Vec<i32>,
    implicit_notables: Vec<(usize, usize)>,
}

pub struct Plando {
    pub game_data: Arc<GameData>,
    pub difficulty_tiers: Vec<DifficultyConfig>,
    maps_vanilla: MapRepositoryWrapper,
    maps_standard: Option<MapRepositoryWrapper>,
    maps_wild: Option<MapRepositoryWrapper>,
    pub map_editor: MapEditor,
    pub randomizer_settings: RandomizerSettings,
    pub objectives: Vec<Objective>,
    pub item_locations: Vec<Item>,
    pub start_location: StartLocation,
    pub placed_item_count: [usize; Placeable::VARIANTS.len()],
    pub randomizable_doors: HashSet<(Option<usize>, Option<usize>)>,
    pub locked_doors: Vec<LockedDoor>,
    pub gray_doors: HashSet<DoorPtrPair>,
    pub spoiler_overrides: Vec<SpoilerOverride>,
    pub custom_escape_time: Option<usize>,
    pub creator_name: String,

    door_lock_loc: Vec<(usize, usize, usize)>,
    door_beam_loc: Vec<(usize, usize, usize)>,
    total_door_count: usize,
    preset_data: ImplicitPresetData,

    logic: Logic,

    pub rng: StdRng
}

impl Plando {
    pub fn new(game_data: GameData, randomizer_settings: RandomizerSettings, preset_data: &PresetData) -> Result<Self> {
        let mut maps_vanilla: MapRepositoryWrapper = Plando::load_map_repository(MapRepositoryType::Vanilla).ok_or(anyhow!("Vanilla Map Repository not found"))?.into();
        let maps_standard = Plando::load_map_repository(MapRepositoryType::Standard).map(|x| x.into());
        let maps_wild = Plando::load_map_repository(MapRepositoryType::Wild).map(|x| x.into());

        if maps_standard.is_none() {
            println!("WARN: Standard Map Repository not found");
        }
        if maps_wild.is_none() {
            println!("WARN: Wild Map Repository not found");
        }
        
        let mut rng = rand::rngs::StdRng::from_entropy();

        let game_data = Arc::new(game_data);

        let map = maps_vanilla.roll_map(&mut rng, &game_data)?;
        let map_editor = MapEditor::new(map, game_data.clone());

        let objectives = maprando::randomize::get_objectives(&randomizer_settings, Some(map_editor.get_map()), &game_data, &mut rng);
        let randomizable_doors = get_randomizable_doors(&game_data, &objectives);

        let ship_start = Self::get_ship_start();

        let mut placed_item_count = [0usize; Placeable::VARIANTS.len()];
        placed_item_count[0] = 1;

        let item_location_len = game_data.item_locations.len();

        let impl_preset_data = ImplicitPresetData {
            difficulty_tiers: preset_data.difficulty_tiers.clone(),
            implicit_tech: preset_data.tech_by_difficulty["Implicit"].clone(),
            implicit_notables: preset_data.notables_by_difficulty["Implicit"].clone()
        };

        let mut plando = Plando {
            game_data: game_data.clone(),
            difficulty_tiers: Vec::new(),
            maps_vanilla,
            maps_standard,
            maps_wild,
            map_editor,
            randomizer_settings,
            objectives,
            item_locations: vec![Item::Nothing; item_location_len],
            start_location: ship_start,
            placed_item_count,
            randomizable_doors,
            locked_doors: Vec::new(),
            gray_doors: get_gray_doors(),
            spoiler_overrides: Vec::new(),
            custom_escape_time: None,
            creator_name: "Plando".to_string(),

            door_lock_loc: Vec::new(),
            door_beam_loc: Vec::new(),
            total_door_count: 0,
            preset_data: impl_preset_data,
            logic: Logic::new(game_data),

            rng
        };
        
        plando.get_difficulty_tiers();

        Ok(plando)
    }

    pub fn map(&self) -> &Map {
        self.map_editor.get_map()
    }

    pub fn get_randomization(&'_ self) -> MutexGuard<'_, Option<(Randomization, SpoilerLog)>> {
        self.logic.get_randomization()
    }

    pub fn get_hub_data(&'_ self) -> MutexGuard<'_, HubLocationData> {
        self.logic.get_hub_data()
    }

    pub fn clear_item_locations(&mut self) {
        for i in 0..self.item_locations.len() {
            self.item_locations[i] = Item::Nothing;
        }
        for i in Placeable::ETank as usize..=Placeable::WalljumpBoots as usize {
            self.placed_item_count[i] = 0;
        }
        self.spoiler_overrides.clear();
    }

    pub fn room_id_to_idx(&self, id: usize) -> usize {
        let room_ptr = self.game_data.room_ptr_by_id[&id];
        self.game_data.room_idx_by_ptr[&room_ptr]
    }

    pub fn _get_door_idx(&self, room_idx: usize, tile_x: usize, tile_y: usize, direction: String) -> Option<usize> {
        let door_opt = self.game_data.room_geometry[room_idx].doors.iter().position(|x| {
            x.direction == direction && x.x == tile_x && x.y == tile_y
        });
        if let Some(door_idx) = door_opt {
            return Some(door_idx);
        }
        None
    }

    pub fn load_map_repository(map_repo_type: MapRepositoryType) -> Option<MapRepository> {
        let vanilla_map_path = Path::new("../maps/vanilla");
        let standard_maps_path = Path::new("../maps/v119-standard-avro");
        let wild_maps_path = Path::new("../maps/v119-wild-avro");

        match map_repo_type {
            MapRepositoryType::Vanilla => MapRepository::new("Vanilla", vanilla_map_path).ok(),
            MapRepositoryType::Standard => MapRepository::new("Standard", standard_maps_path).ok(),
            MapRepositoryType::Wild => MapRepository::new("Wild", wild_maps_path).ok()
        }
    }

    pub fn reload_map_repositories(&mut self) {
        self.maps_standard = Self::load_map_repository(MapRepositoryType::Standard).map(|x| x.into());
        self.maps_wild = Self::load_map_repository(MapRepositoryType::Standard).map(|x| x.into());
    }

    pub fn load_map(&mut self, map: Map) {
        self.map_editor.load_map(map);
        self.clear_item_locations();
        self.clear_doors();
        self.start_location = Plando::get_ship_start();
        self.update_randomizable_doors();
        self.logic.reset();
    }

    pub fn load_map_from_file(&mut self, path: &Path) -> Result<()> {
        self.map_editor.load_map_from_file(path)?;
        self.clear_item_locations();
        self.clear_doors();
        self.start_location = Plando::get_ship_start();
        self.update_randomizable_doors();
        self.logic.reset();
        Ok(())
    }

    pub fn patch_rom(&mut self, rom_vanilla: &Rom, settings: CustomizeSettings, samus_sprite_categories: Vec<SamusSpriteCategory>, mosaic_themes: Vec<MosaicTheme>) -> Result<JoinHandle<Result<Rom>>> {
        if self.map_editor.error_list.iter().filter(|err| err.is_severe()).count() > 0 {
            bail!("Map has errors that need to be fixed");
        }

        // Place any remaining wall doors
        for door in self.map_editor.error_list.clone() {
            if let MapErrorType::DoorDisconnected(room_idx, door_idx) = door {
                if let Err(e) = self.place_door(room_idx, door_idx, Some(DoorType::Wall), false) {
                    let door = &self.game_data.room_geometry[room_idx].doors[door_idx];
                    let ptr_pair = (door.exit_ptr, door.entrance_ptr);
                    let (room_id, node_id) = self.game_data.door_ptr_pair_map[&ptr_pair];
                    let room_name = self.game_data.room_json_map[&room_id]["name"].as_str().unwrap();
                    let node_name = self.game_data.node_json_map[&(room_id, node_id)]["name"].as_str().unwrap();
                    bail!("Could not place wall door: {room_name}: {node_name} - {e}");
                }
            }
        }

        let handle = self.update_spoiler_data()?;
        let randomizer_settings = self.randomizer_settings.clone();
        let arc = self.logic.get_randomization_arc();

        let game_data = self.game_data.clone();
        let rom_vanilla = rom_vanilla.clone();

        let result_handle: JoinHandle<Result<Rom>> = tokio::spawn(async move {
            handle.await??;

            let randomization = arc.lock().unwrap();
            if randomization.is_none() {
                bail!("Could not generate spoiler data");
            }
            let (r, _) = randomization.as_ref().unwrap();

            maprando::patch::make_rom(
                &rom_vanilla,
                &randomizer_settings,
                &settings,
                r,
                &game_data,
                &samus_sprite_categories,
                &mosaic_themes
            )
        });

        Ok(result_handle)
    }

    pub fn does_repo_exist(&self, repo_type: MapRepositoryType) -> bool {
        match repo_type {
            MapRepositoryType::Vanilla => true,
            MapRepositoryType::Standard => self.maps_standard.is_some(),
            MapRepositoryType::Wild => self.maps_wild.is_some()
        }
    }

    pub fn reroll_map(&mut self, map_repository: MapRepositoryType) -> Result<()> {
        let repo = match map_repository {
            MapRepositoryType::Vanilla => &mut self.maps_vanilla,
            MapRepositoryType::Standard => self.maps_standard.as_mut().unwrap(),
            MapRepositoryType::Wild => self.maps_wild.as_mut().unwrap()
        };

        let map = repo.roll_map(&mut self.rng, &self.game_data)?;
        self.load_map(map);
        
        Ok(())
    }

    pub fn load_preset(&mut self, preset: RandomizerSettings) {
        self.randomizer_settings = preset;
        self.objectives = maprando::randomize::get_objectives(&self.randomizer_settings, Some(self.map_editor.get_map()), &self.game_data, &mut self.rng);
        self.update_randomizable_doors();
        self.get_difficulty_tiers();
    }

    pub fn update_randomizable_doors(&mut self) {
        self.randomizable_doors = get_randomizable_doors(&self.game_data, &self.objectives);
    }

    pub fn get_difficulty_tiers(&mut self) {
        self.difficulty_tiers = maprando::randomize::get_difficulty_tiers(
            &self.randomizer_settings, 
            &self.preset_data.difficulty_tiers, 
            &self.game_data, 
            &self.preset_data.implicit_tech,
            &self.preset_data.implicit_notables
        );
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

    pub fn get_ship_hub(game_data: &GameData) -> HubLocation {
        HubLocation {
            room_id: 8,
            node_id: 5,
            vertex_id: game_data.vertex_isv.index_by_key[&VertexKey {
                room_id: 8,
                node_id: 5,
                ..Default::default()
            }]
        }
    }

    pub fn erase_room(&mut self, room_idx: usize) {
        let room_id = self.game_data.room_geometry[room_idx].room_id;

        // Ensure Landing Site and Motherbrain cannot be erased
        if room_id == 8 || room_id == 238 {
            return;
        }

        // Clear all items in the room so they can be placed again
        for item_idx in 0..self.item_locations.len() {
            if self.item_locations[item_idx] != Item::Nothing && self.game_data.item_locations[item_idx].0 == room_id {
                self.place_item(item_idx, Item::Nothing);
            }
        }

        // Clear all door locks
        for door_idx in 0..self.game_data.room_geometry[room_idx].doors.len() {
            // This should never error
            let _ = self.place_door(room_idx, door_idx, None, true);
        }

        // Reset start location if its inside removed room
        if self.start_location.room_id == room_id {
            self.place_start_location(Self::get_ship_start());
        }

        self.map_editor.erase_room(room_idx, &self.locked_doors);
    }

    pub fn place_start_location(&mut self, start_loc: StartLocation) {
        self.start_location = start_loc;
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
    }

    pub fn place_door(&mut self, room_idx: usize, door_idx: usize, door_type_opt: Option<DoorType>, replace: bool) -> Result<()> {
        let door = &self.game_data.room_geometry[room_idx].doors[door_idx];
        let ptr_pair = (door.exit_ptr, door.entrance_ptr);

        let door_conn_opt = self.map().doors.iter().find(|(src, dst, _)| *src == ptr_pair || *dst == ptr_pair).cloned();
        let (src_ptr_pair, dst_ptr_pair) = if let Some(door_conn) = door_conn_opt {
            (door_conn.0, door_conn.1)
        } else {
            (ptr_pair, (None, None))
        };

        // Door if unlinked, or Door connection if linked is not randomizable
        if !(self.randomizable_doors.contains(&src_ptr_pair) && (door_conn_opt.is_none() || self.randomizable_doors.contains(&dst_ptr_pair))) {
            bail!("Door is not randomizable. Non-randomizable doors include Gray Doors (like bosses), Sandpits, doors on the same tile as an item and doors to Save/Map/Refill rooms");
        }

        let (src_room_idx, src_door_idx) = self.game_data.room_and_door_idxs_by_door_ptr_pair[&src_ptr_pair];
        let tile_src_x = self.game_data.room_geometry[src_room_idx].doors[src_door_idx].x;
        let tile_src_y = self.game_data.room_geometry[src_room_idx].doors[src_door_idx].y;
        let loc_src = (src_room_idx, tile_src_x, tile_src_y);

        let loc_dst = if dst_ptr_pair.0.is_none() && dst_ptr_pair.1.is_none() {
            None
        } else {
            let (dst_room_idx, dst_door_idx) = self.game_data.room_and_door_idxs_by_door_ptr_pair[&dst_ptr_pair];
            let tile_dst_x = self.game_data.room_geometry[dst_room_idx].doors[dst_door_idx].x;
            let tile_dst_y = self.game_data.room_geometry[dst_room_idx].doors[dst_door_idx].y;
            Some((dst_room_idx, tile_dst_x, tile_dst_y))
        };

        let prev_door_opt = self.locked_doors.iter().find(|door| door.src_ptr_pair == src_ptr_pair || door.dst_ptr_pair == src_ptr_pair);

        // We can always remove a door lock
        if door_type_opt.is_none() {
            // This confirms that we remove the actually targetted door, not another door on the same tile
            if prev_door_opt.is_none() {
                return Ok(());
            }

            let prev_door = prev_door_opt.unwrap();
            let prev_placeable = Placeable::from_door_type(prev_door.door_type).unwrap();

            self.door_lock_loc.retain(|&elem| {
                elem != loc_src && loc_dst.is_none_or(|x| elem != x)
            });
            self.door_beam_loc.retain(|&elem| {
                elem != loc_src && loc_dst.is_none_or(|x| elem != x)
            });

            self.placed_item_count[prev_placeable as usize] -= 1;
            if prev_door.door_type != DoorType::Wall {
                self.total_door_count -= 1;
            }

            self.locked_doors.retain(|x| x.src_ptr_pair != src_ptr_pair || x.dst_ptr_pair != dst_ptr_pair);

            self.map_editor.is_valid(&self.locked_doors);

            return Ok(());
        }
        let door_type = door_type_opt.unwrap();

        if self.total_door_count >= 55 && door_type != DoorType::Wall {
            bail!("Cannot place more than 55 door locks (Wall doors don't count towards this total)");
        }

        // At this point the door may still be unlinked. In this case we only allow placing Wall Doors
        if door_conn_opt.is_none() && door_type != DoorType::Wall {
            bail!("Cannot place door locks on unlinked doors except Wall doors");
        }

        let placeable = Placeable::from_door_type(door_type).unwrap();

        // There is already a door lock where we try to place one. If replace is false we simply return and don't throw an error
        if prev_door_opt.is_some() && !replace {
            return Ok(());
        }

        if door_type != DoorType::Wall {
            // Check that there is not already a door on this tile
            if self.door_lock_loc.contains(&loc_src) || loc_dst.is_some_and(|x| self.door_lock_loc.contains(&x)) {
                bail!("There can only be one door lock per tile");
            }

            // Check that there is only one beam door per room
            if let DoorType::Beam(_) = door_type && self.door_beam_loc.iter().any(|&x| {
                x.0 == loc_src.0 || loc_dst.is_some_and(|y| x.0 == y.0)
            }) {
                bail!("There can only be one beam door per room");
            }
        }

        // Remove any previous door locks
        if prev_door_opt.is_some() {
            self.place_door(room_idx, door_idx, None, false)?;
        }

        // Actually place the door
        self.door_lock_loc.push(loc_src);
        if let Some(dst) = loc_dst {
            self.door_lock_loc.push(dst);
        }

        if let DoorType::Beam(_) = door_type {
            self.door_beam_loc.push(loc_src);
            if let Some(dst) = loc_dst {
                self.door_beam_loc.push(dst);
            }
        }

        let locked_door = LockedDoor {
            src_ptr_pair, dst_ptr_pair, door_type,
            bidirectional: door_conn_opt.is_some()
        };

        self.locked_doors.push(locked_door);
        self.map_editor.is_valid(&self.locked_doors);

        self.placed_item_count[placeable as usize] += 1;
        if door_type != DoorType::Wall {
            self.total_door_count += 1;
        }

        Ok(())
    }

    pub fn clear_doors(&mut self) {
        self.door_beam_loc.clear();
        self.door_lock_loc.clear();
        self.locked_doors.clear();
        self.total_door_count = 0;

        for i in Placeable::DoorMissile as usize..=Placeable::DoorWall as usize {
            self.placed_item_count[i] = 0;
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
            let count = match placeable {
                Placeable::Missile => return None,
                Placeable::SuperMissile => return None,
                Placeable::PowerBomb => return None,
                Placeable::ETank => 14,
                Placeable::ReserveTank => 4,
                _ => 0
            };
            let item = placeable.to_item().unwrap();
            if let Some(item_count) = self.randomizer_settings.item_progression_settings.starting_items.iter().find(|x| x.item == item) {
                if count < item_count.count {
                    return Some(0);
                }
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

    pub fn update_overrides(&mut self) {
        self.spoiler_overrides.retain(|x| {
            self.item_locations[x.item_idx] != Item::Nothing
        });
    }

    pub fn is_map_logic_valid(&self) -> Result<()> {
        // Ensure there are enough available item locations. Randomizer::new has an assert on this
        let map = self.map();
        let item_loc_count = self.game_data.item_locations.iter().filter(|(room_id, _)| {
            let room_idx = self.room_id_to_idx(*room_id);
            map.room_mask[room_idx]
        }).count();
        let start_items = &self.randomizer_settings.item_progression_settings.starting_items;
        let start_tanks = start_items.iter().map(
            |i| if i.item == Item::ETank || i.item == Item::ReserveTank { i.count } else { 0 }
        ).sum::<usize>();

        if item_loc_count + start_tanks < 13 {
            bail!("Not enough available item locations. Need at least {}", 13 - start_tanks);
        }

        // Ensure certain doors (if placed) are always linked
        let needed_doors = vec![
            (321, 1), (321, 2), // Toilet top and bottom, because logic creates a link that skips over toilet
            (32, 1), // West Ocean Bottom Left Door, because logic links the bridge door
            (32, 5) // West Ocean Bottom Right Door, because logic links the bridge door
        ];

        for door in needed_doors {
            let room_idx = self.room_id_to_idx(door.0);
            if !self.map().room_mask[room_idx] {
                continue;
            }

            let door_ptr_pair = self.game_data.reverse_door_ptr_pair_map[&door];

            if !self.map().doors.iter().any(|x| x.0 == door_ptr_pair || x.1 == door_ptr_pair) {
                let room_geometry = &self.game_data.room_geometry[room_idx];
                let room_name = &room_geometry.name;
                let node_name = self.game_data.node_json_map[&door]["name"].as_str().unwrap();
                bail!("Door needs connection for logic to be calculated: {room_name}: {node_name}");
            }
        }

        Ok(())
    }

    pub fn update_settings(&mut self) {
        let settings = &mut self.randomizer_settings;

        let empty_pool = (0..self.game_data.item_isv.keys.len()).map(|idx| {
            ItemCount {
                item: Item::try_from(idx).unwrap(),
                count: 0
            }
        }).collect();
        settings.item_progression_settings.item_pool = empty_pool;

        settings.item_progression_settings.preset = Some(self.creator_name.clone());
        settings.map_layout = self.creator_name.clone();

        settings.doors_mode = DoorsMode::Beam;
    }

    pub fn update_spoiler_data(&mut self) -> Result<JoinHandle<Result<()>>> {
        if let Err(err) = self.is_map_logic_valid() {
            self.logic.reset();
            return Err(err);
        }

        self.update_settings();
        self.update_overrides();

        let locked_door_data = self.get_locked_door_data();
        let implicit_tech = &self.preset_data.implicit_tech;
        let implicit_notables = &self.preset_data.implicit_notables;
        let difficulty = DifficultyConfig::new(
            &self.randomizer_settings.skill_assumption_settings,
            &self.game_data,
            &implicit_tech,
            &implicit_notables,
        );

        let (initial_global_state, initial_local_state) = self.get_initial_states();

        let handle = self.logic.update_hub_and_randomization(
            initial_global_state,
            initial_local_state,
            self.start_location.clone(),
            locked_door_data,
            self.objectives.clone(),
            difficulty,
            self.item_locations.clone(),
            self.spoiler_overrides.clone(),
            self.randomizer_settings.clone(),
            self.difficulty_tiers.clone(),
            self.map().clone(),
            self.custom_escape_time.clone()
        );
        
        Ok(handle)
    }

    /* COPY FROM maprando::randomize::get_initial_states */
    fn get_initial_states(&self) -> (GlobalState, LocalState) {
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
        let mut local = LocalState::empty(&global);
        for x in &self.randomizer_settings.item_progression_settings.starting_items {
            for _ in 0..x.count {
                global.collect(
                    x.item,
                    &self.game_data,
                    self.randomizer_settings
                        .item_progression_settings
                        .ammo_collect_fraction,
                    &self.difficulty_tiers[0].tech,
                    &mut local
                );
            }
        }
        (global, local)
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