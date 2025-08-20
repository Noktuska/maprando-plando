use std::time::SystemTime;

use hashbrown::{HashMap, HashSet};
use maprando::{randomize::{escape_timer::{self, SpoilerEscape}, DebugData, EssentialItemSpoilerInfo, EssentialSpoilerData, Randomization, RandomizationState, Randomizer, SpoilerDetails, SpoilerItemLoc, SpoilerLocation, SpoilerLog, SpoilerRoomLoc, SpoilerStartLocation, SpoilerSummary}, settings::{Objective, RandomizerSettings, SaveAnimals, WallJump}, traverse::get_bireachable_idxs};
use maprando_game::{DoorPtrPair, GameData, Item, Map, NodeId, RoomId, VertexKey};
use rand::{rngs::StdRng, Rng, SeedableRng};
use strum::VariantNames;

pub struct VertexInfo {
    pub area_name: String,
    pub room_id: usize,
    pub room_name: String,
    pub room_coords: (usize, usize),
    pub node_name: String,
    pub node_id: usize,
}

pub fn get_vertex_info(vertex_id: usize, game_data: &GameData, map: &Map) -> VertexInfo {
    let VertexKey {
        room_id, node_id, ..
    } = game_data.vertex_isv.keys[vertex_id];
    get_vertex_info_by_id(room_id, node_id, game_data, map)
}

pub fn get_vertex_info_by_id(room_id: RoomId, node_id: NodeId, game_data: &GameData, map: &Map) -> VertexInfo {
    let room_ptr = game_data.room_ptr_by_id[&room_id];
    let room_idx = game_data.room_idx_by_ptr[&room_ptr];
    let area = map.area[room_idx];
    let room_coords = map.rooms[room_idx];
    VertexInfo {
        area_name: game_data.area_names[area].clone(),
        room_name: game_data.room_json_map[&room_id]["name"]
            .as_str()
            .unwrap()
            .to_string(),
        room_id,
        room_coords,
        node_name: game_data.node_json_map[&(room_id, node_id)]["name"]
            .as_str()
            .unwrap()
            .to_string(),
        node_id,
    }
}

pub fn get_gray_doors() -> HashSet<DoorPtrPair> {
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

pub fn get_randomizable_doors(game_data: &GameData, objectives: &[Objective]) -> HashSet<DoorPtrPair> {
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

// This is an exact copy and paste of Randomizer::get_randomization, except it does not bail on an invalid escape route
pub fn get_escape_safe_randomization(
    r: &Randomizer,
    state: &RandomizationState,
    spoiler_summaries: Vec<SpoilerSummary>,
    spoiler_details: Vec<SpoilerDetails>,
    mut debug_data_vec: Vec<DebugData>,
    seed: usize,
    display_seed: usize,
    rng: &mut StdRng
) -> (Randomization, SpoilerLog) {
    // Compute the first step on which each node becomes reachable/bireachable:
    let mut node_reachable_step: HashMap<(RoomId, NodeId), usize> = HashMap::new();
    let mut node_bireachable_step: HashMap<(RoomId, NodeId), usize> = HashMap::new();
    let mut map_tile_reachable_step: HashMap<(RoomId, (usize, usize)), usize> = HashMap::new();
    let mut map_tile_bireachable_step: HashMap<(RoomId, (usize, usize)), usize> =
        HashMap::new();

    for (step, debug_data) in debug_data_vec.iter_mut().enumerate() {
        for (
            v,
            VertexKey {
                room_id, node_id, ..
            },
        ) in r.game_data.vertex_isv.keys.iter().enumerate()
        {
            if node_bireachable_step.contains_key(&(*room_id, *node_id)) {
                continue;
            }
            if get_bireachable_idxs(
                &debug_data.global_state,
                v,
                &debug_data.forward,
                &debug_data.reverse,
            )
            .is_some()
            {
                node_bireachable_step.insert((*room_id, *node_id), step);
                let room_ptr = r.game_data.room_ptr_by_id[room_id];
                let room_idx = r.game_data.room_idx_by_ptr[&room_ptr];
                if let Some(coords) = r.game_data.node_tile_coords.get(&(*room_id, *node_id))
                {
                    for (x, y) in coords.iter().copied() {
                        let key = if *room_id == 322 {
                            // Adjust for East Pants Room being offset by one screen right and down from Pants Room
                            (room_idx, (x + 1, y + 1))
                        } else if *room_id == 313 {
                            // Adjust Homing Geemer Room being offset from West Ocean:
                            (room_idx, (x + 5, y + 2))
                        } else {
                            (room_idx, (x, y))
                        };
                        if !map_tile_bireachable_step.contains_key(&key) {
                            map_tile_bireachable_step.insert(key, step);
                        }
                    }
                }
            }

            if node_reachable_step.contains_key(&(*room_id, *node_id)) {
                continue;
            }
            if debug_data.forward.cost[v]
                .iter()
                .any(|&x| f32::is_finite(x))
            {
                node_reachable_step.insert((*room_id, *node_id), step);
                let room_ptr = r.game_data.room_ptr_by_id[room_id];
                let room_idx = r.game_data.room_idx_by_ptr[&room_ptr];
                if let Some(coords) = r.game_data.node_tile_coords.get(&(*room_id, *node_id))
                {
                    for (x, y) in coords.iter().copied() {
                        let key = if *room_id == 322 {
                            // Adjust for East Pants Room being offset by one screen right and down from Pants Room
                            (room_idx, (x + 1, y + 1))
                        } else if *room_id == 313 {
                            // Adjust Homing Geemer Room being offset from West Ocean:
                            (room_idx, (x + 5, y + 2))
                        } else {
                            (room_idx, (x, y))
                        };
                        if !map_tile_reachable_step.contains_key(&key) {
                            map_tile_reachable_step.insert(key, step);
                        }
                    }
                }
            }
        }
    }

    let item_placement: Vec<Item> = state
        .item_location_state
        .iter()
        .map(|x| x.placed_item.unwrap())
        .collect();
    let spoiler_all_items = state
        .item_location_state
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let (room, n) = r.game_data.item_locations[i];
            let item_vertex_info = get_vertex_info_by_id(room, n, &r.game_data, &r.map);
            let room_id = item_vertex_info.room_id;
            let node_id = item_vertex_info.node_id;
            let node_coords = r.game_data.node_coords[&(room_id, node_id)];
            let coords = (
                item_vertex_info.room_coords.0 + node_coords.0,
                item_vertex_info.room_coords.1 + node_coords.1,
            );
            let location = SpoilerLocation {
                area: item_vertex_info.area_name,
                room_id,
                room: item_vertex_info.room_name,
                node_id,
                node: item_vertex_info.node_name,
                coords,
            };
            let item = x.placed_item.unwrap();
            SpoilerItemLoc {
                item: Item::VARIANTS[item as usize].to_string(),
                location,
            }
        })
        .collect();

    let mut spoiler_all_rooms: Vec<SpoilerRoomLoc> = Vec::new();
    for (room_idx, room_coords) in r.map.rooms.iter().enumerate() {
        if !r.map.room_mask[room_idx] {
            continue;
        }
        let room_geom = &r.game_data.room_geometry[room_idx];
        let room_id = r.game_data.room_id_by_ptr[&room_geom.rom_address];
        let room = r.game_data.room_json_map[&room_id]["name"]
            .as_str()
            .unwrap()
            .to_string();
        let map = if room_idx == r.game_data.toilet_room_idx {
            vec![vec![1; 1]; 10]
        } else {
            room_geom.map.clone()
        };
        let height = map.len();
        let width = map[0].len();
        let mut map_reachable_step: Vec<Vec<u8>> = vec![vec![255; width]; height];
        let mut map_bireachable_step: Vec<Vec<u8>> = vec![vec![255; width]; height];
        for y in 0..height {
            for x in 0..width {
                if map[y][x] != 0 {
                    let key = (room_idx, (x, y));
                    if let Some(step) = map_tile_reachable_step.get(&key) {
                        map_reachable_step[y][x] = *step as u8;
                    }
                    if let Some(step) = map_tile_bireachable_step.get(&key) {
                        map_bireachable_step[y][x] = *step as u8;
                    }
                }
            }
        }
        spoiler_all_rooms.push(SpoilerRoomLoc {
            room_id,
            room,
            map,
            map_reachable_step,
            map_bireachable_step,
            coords: *room_coords,
        });
    }

    let save_animals = if r.settings.save_animals == SaveAnimals::Random {
        if rng.gen_bool(0.5) {
            SaveAnimals::Yes
        } else {
            SaveAnimals::No
        }
    } else {
        r.settings.save_animals
    };

    // Compute a default escape timer if the base one fails
    let spoiler_escape = escape_timer::compute_escape_data(
        r.game_data,
        r.map,
        r.settings,
        save_animals != SaveAnimals::No,
        &r.difficulty_tiers[0],
    ).unwrap_or_else(|_| {
        SpoilerEscape {
            base_igt_frames: 0,
            base_igt_seconds: 0.0,
            base_leniency_factor: 1.0,
            difficulty_multiplier: r.settings.skill_assumption_settings.escape_timer_multiplier,
            raw_time_seconds: 0.0,
            final_time_seconds: 0.0,
            animals_route: None,
            ship_route: Vec::new()
        }
    });

    let spoiler_objectives: Vec<String> = r
        .objectives
        .iter()
        .map(|x| x.get_flag_name().to_owned())
        .collect();

    let hub_room_id = state.hub_location.room_id;
    let hub_room_name = r.game_data.room_json_map[&hub_room_id]["name"]
        .as_str()
        .unwrap()
        .to_string();
    let spoiler_log = SpoilerLog {
        item_priority: state
            .item_precedence
            .iter()
            .map(|x| format!("{x:?}"))
            .collect(),
        summary: spoiler_summaries,
        objectives: spoiler_objectives,
        start_location: SpoilerStartLocation {
            name: state.start_location.name.clone(),
            room_id: state.start_location.room_id,
            node_id: state.start_location.node_id,
            x: state.start_location.x,
            y: state.start_location.y,
        },
        hub_location_name: hub_room_name,
        hub_obtain_route: state.hub_obtain_route.clone(),
        hub_return_route: state.hub_return_route.clone(),
        escape: spoiler_escape,
        details: spoiler_details,
        all_items: spoiler_all_items,
        all_rooms: spoiler_all_rooms,
    };

    let randomization = Randomization {
        objectives: r.objectives.clone(),
        save_animals,
        map: r.map.clone(),
        toilet_intersections: r.toilet_intersections.clone(),
        locked_doors: r.locked_door_data.locked_doors.clone(),
        item_placement,
        escape_time_seconds: spoiler_log.escape.final_time_seconds,
        essential_spoiler_data: get_essential_spoiler_data(r.settings, &spoiler_log),
        seed,
        display_seed,
        seed_name: get_seed_name(seed),
        start_location: state.start_location.clone(),
    };
    (randomization, spoiler_log)
}

fn get_seed_name(seed: usize) -> String {
    let t = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut rng_seed = [0u8; 32];
    rng_seed[..8].copy_from_slice(&seed.to_le_bytes());
    rng_seed[8..24].copy_from_slice(&t.to_le_bytes());
    let mut rng = rand::rngs::StdRng::from_seed(rng_seed);
    // Leave out vowels and characters that could read like vowels, to minimize the chance
    // of forming words.
    let alphabet = "256789BCDFGHJKLMNPQRSTVWXYZbcdfghjkmnpqrstvwxyz";
    let mut out: String = String::new();
    let num_chars = 9;
    for _ in 0..num_chars {
        let i = rng.gen_range(0..alphabet.len());
        let c = alphabet.as_bytes()[i] as char;
        out.push(c);
    }
    out
}

fn get_essential_spoiler_data(
    settings: &RandomizerSettings,
    spoiler_log: &SpoilerLog,
) -> EssentialSpoilerData {
    let mut item_spoiler_info: Vec<EssentialItemSpoilerInfo> = vec![];
    let mut items_set: HashSet<Item> = HashSet::new();

    // Include starting items first, as "step 0":
    for x in &settings.item_progression_settings.starting_items {
        if x.count > 0 {
            item_spoiler_info.push(EssentialItemSpoilerInfo {
                item: x.item,
                step: Some(0),
                area: None,
            });
            items_set.insert(x.item);
        }
    }

    // Include collectible items in the middle:
    for (step, step_summary) in spoiler_log.summary.iter().enumerate() {
        for item_info in step_summary.items.iter() {
            let item = Item::try_from(item_info.item.as_str()).unwrap();
            if !items_set.contains(&item) {
                item_spoiler_info.push(EssentialItemSpoilerInfo {
                    item,
                    step: Some(step + 1),
                    area: Some(item_info.location.area.clone()),
                });
                items_set.insert(item);
            }
        }
    }

    // Include logically uncollectible items:
    for loc in &spoiler_log.all_items {
        if loc.item == "Nothing" {
            continue;
        }
        let item = Item::try_from(loc.item.as_str()).unwrap();
        if !items_set.contains(&item) {
            item_spoiler_info.push(EssentialItemSpoilerInfo {
                item,
                step: None,
                area: Some(loc.location.area.clone()),
            });
            items_set.insert(item);
        }
    }

    // Include unplaced items at the end:
    for &name in Item::VARIANTS {
        if name == "Nothing" {
            continue;
        }
        if settings.other_settings.wall_jump != WallJump::Collectible && name == "WallJump" {
            // Don't show "WallJump" item unless using Collectible mode.
            continue;
        }
        let item = Item::try_from(name).unwrap();
        if !items_set.contains(&item) {
            item_spoiler_info.push(EssentialItemSpoilerInfo {
                item,
                step: None,
                area: None,
            });
            items_set.insert(item);
        }
    }

    EssentialSpoilerData { item_spoiler_info }
}