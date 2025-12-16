use std::{io::Write, path::Path};

use anyhow::{anyhow, bail, Result};
use hashbrown::HashSet;
use maprando::{preset::PresetData, randomize::LockedDoor, settings::{try_upgrade_settings, NotableSetting, Objective, ObjectiveSetting, RandomizerSettings, TechSetting}};
use maprando_game::{BeamType, DoorType, GameData, Item, Map};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{Placeable, Plando, SpoilerOverride};

#[derive(Clone, Serialize, Deserialize)]
pub struct SeedData {
    pub map: Map,
    pub start_location: (usize, usize), // RoomId, NodeId
    pub item_placements: Vec<Item>,
    pub door_locks: Vec<LockedDoor>,
    pub settings: RandomizerSettings,
    #[serde(default = "Vec::new")]
    pub spoiler_overrides: Vec<SpoilerOverride>,
    #[serde(default)]
    pub custom_escape_time: Option<usize>,
    #[serde(default)]
    pub creator_name: String
}

impl SeedData {
    pub const FORMAT: &'static str = "0.2.2";

    pub fn from_plando(plando: &Plando) -> Self {
        Self {
            map: plando.map().clone(),
            start_location: (plando.start_location.room_id, plando.start_location.node_id),
            item_placements: plando.item_locations.clone(),
            door_locks: plando.locked_doors.clone(),
            settings: plando.randomizer_settings.clone(),
            spoiler_overrides: plando.spoiler_overrides.clone(),
            custom_escape_time: plando.custom_escape_time.clone(),
            creator_name: plando.creator_name.clone()
        }
    }

    pub fn from_bytes(data: Vec<u8>, game_data: &GameData, preset_data: &PresetData) -> Result<SeedData> {
        let s = String::from_utf8(data)?;
        let v: Value = serde_json::to_value(&s)?;
        Self::from_json(v, game_data, preset_data)
    }

    pub fn from_file(path: &Path, game_data: &GameData, preset_data: &PresetData) -> Result<SeedData> {
        let s = std::fs::read_to_string(path)?;
        let v: Value = serde_json::to_value(&s)?;
        Self::from_json(v, game_data, preset_data)
    }

    pub fn load_into_plando(self, plando: &mut Plando) -> Result<()> {
        plando.load_preset(self.settings);

        plando.custom_escape_time = self.custom_escape_time;
        plando.creator_name = self.creator_name;

        plando.load_map(self.map);

        plando.item_locations = self.item_placements;
        for item in &plando.item_locations {
            if *item != Item::Nothing {
                plando.placed_item_count[*item as usize + Placeable::ETank as usize] += 1;
            }
        }
        
        for door_data in self.door_locks {
            let (room_idx, door_idx) = plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door_data.src_ptr_pair];

            plando.place_door(room_idx, door_idx, Some(door_data.door_type), false)?;
        }

        let ship_start = Plando::get_ship_start();
        let start_loc = if self.start_location == (ship_start.room_id, ship_start.node_id) {
            ship_start
        } else {
            let start_loc_idx = plando.game_data.start_location_id_map[&self.start_location];
            plando.game_data.start_locations[start_loc_idx].clone()
        };
        plando.place_start_location(start_loc);

        plando.spoiler_overrides = self.spoiler_overrides;

        Ok(())
    }

    fn from_json(mut v: Value, game_data: &GameData, preset_data: &PresetData) -> Result<SeedData> {
        // Try parsing an older version (<= v0.2.1)
        if let Ok(r) = Self::from_json_legacy(v.clone(), game_data, preset_data) {
            return Ok(r);
        }

        // Uncompact room_mask from Bitmask to a Vec<bool>
        let map = get_key(&mut v, "map")?;
        let room_mask_v = get_key(map, "room_mask")?;
        // v119: room_mask is not compressed
        if as_arr(room_mask_v)?.len() < game_data.room_geometry.len() {
            let room_mask: Vec<u8> = serde_json::from_value(room_mask_v.take())?;
            let mut room_mask_out = Vec::with_capacity(room_mask.len() * 8);
            for val in room_mask {
                for offset in 0..8 {
                    let b = val & (1 << offset);
                    if b > 0 {
                        room_mask_out.push(true);
                    } else {
                        room_mask_out.push(false);
                    }
                }
            }
            *room_mask_v = serde_json::to_value(room_mask_out)?;
        }

        // Insert omitted fields into door_locks
        let door_locks = get_key(&mut v, "door_locks")?;
        let door_locks_arr = as_arr(door_locks)?;
        // v119: door_locks are not compressed
        if door_locks_arr.first().map(|lock| serde_json::from_value::<LockedDoor>(lock.clone()).is_err()).unwrap_or(false) {
            for door_lock in door_locks_arr {
                let door_lock_obj = as_obj(door_lock)?;
                door_lock_obj.insert("dst_ptr_pair".to_string(), json!([null, null]));
                door_lock_obj.insert("bidirectional".to_string(), Value::Bool(false));
            }
        }

        // Expand tech and notable settings
        let settings = get_key(&mut v, "settings")?;
        let skill_assumption = get_key(settings, "skill_assumption_settings")?;
        let tech_settings_v = get_key(skill_assumption, "tech_settings")?;
        let tech_settings: HashSet<i32> = serde_json::from_value(tech_settings_v.take())?;
        let tech_full: Vec<_> = preset_data.tech_data_map.iter().map(|(_, data)| {
            TechSetting {
                id: data.tech_id,
                name: data.name.clone(),
                enabled: tech_settings.contains(&data.tech_id)
            }
        }).collect();
        *tech_settings_v = serde_json::to_value(tech_full)?;
        // notable
        let notable_settings_v = get_key(skill_assumption, "notable_settings")?;
        let notable_settings: HashSet<(usize, usize)> = serde_json::from_value(notable_settings_v.take())?;
        let notable_full: Vec<_> = preset_data.notable_data_map.iter().map(|(_, data)| {
            NotableSetting {
                room_id: data.room_id,
                room_name: data.room_name.clone(),
                notable_id: data.notable_id,
                notable_name: data.name.clone(),
                enabled: notable_settings.contains(&(data.room_id, data.notable_id))
            }
        }).collect();
        *notable_settings_v = serde_json::to_value(notable_full)?;

        // Expand item progression
        let item_progression = get_key(settings, "item_progression_settings")?;
        let Value::Object(item_progression_obj) = item_progression.take() else {
            bail!("Expected value to be of type object")
        };
        *item_progression = serde_json::to_value(&preset_data.default_preset.item_progression_settings)?;
        for (k, v) in item_progression_obj {
            *get_key(item_progression, &k)? = v;
        }

        // Expand objective settings
        let objective_settings = get_key(settings, "objective_settings")?;
        let objective_options = get_key(objective_settings, "objective_options")?;
        let objective_settings: HashSet<Objective> = serde_json::from_value(objective_options.take())?;
        let mut objective_full = preset_data.default_preset.objective_settings.objective_options.clone();
        for obj in &mut objective_full {
            obj.setting = if objective_settings.contains(&obj.objective) {
                ObjectiveSetting::Yes
            } else {
                ObjectiveSetting::No
            };
        }
        *objective_options = serde_json::to_value(objective_full)?;

        // Upgrade settings
        let settings_str = settings.take().to_string();
        let settings_upgraded = try_upgrade_settings(settings_str, preset_data, false)?.0;
        *settings = serde_json::from_str(&settings_upgraded)?;

        let seed_data: SeedData = serde_json::from_value(v)?;

        Ok(seed_data)
    }

    fn from_json_legacy(mut v: Value, game_data: &GameData, preset_data: &PresetData) -> Result<SeedData> {
        let settings = get_key(&mut v, "settings")?;
        let settings_str = settings.take().to_string();
        let settings_upgrade: String = try_upgrade_settings(settings_str, preset_data, false)?.0;
        *settings = serde_json::from_str(&settings_upgrade)?;

        // Start Location was stored as the vec idx in previous editions
        let start_loc_idx = get_key(&mut v, "start_location")?;
        if let Some(idx) = start_loc_idx.as_u64() {
            let start_loc = if idx as usize >= game_data.start_locations.len() {
                &Plando::get_ship_start()
            } else {
                &game_data.start_locations[idx as usize]
            };
            *start_loc_idx = serde_json::to_value((start_loc.room_id, start_loc.node_id))?;
        }

        // Locked doors were put in a serializable wrapper
        let map: Map = serde_json::from_value(v["map"].clone())?;
        let v_door_locks = get_key(&mut v, "door_locks")?;
        let vec = as_arr(v_door_locks)?;
        if !vec.is_empty() && serde_json::from_value::<LockedDoor>(vec[0].clone()).is_err() {
            for old_value in vec {
                let room_id = old_value["room_id"].as_u64().ok_or(anyhow!("Expected room_id"))? as usize;
                let node_id = old_value["node_id"].as_u64().ok_or(anyhow!("Expected node_id"))? as usize;
                let door_type = match old_value["door_type"].as_u64().ok_or(anyhow!("Expected door_type"))? {
                    2 => DoorType::Red,
                    3 => DoorType::Green,
                    4 => DoorType::Yellow,
                    5 => DoorType::Beam(BeamType::Charge),
                    6 => DoorType::Beam(BeamType::Ice),
                    7 => DoorType::Beam(BeamType::Wave),
                    8 => DoorType::Beam(BeamType::Spazer),
                    9 => DoorType::Beam(BeamType::Plasma),
                    _ => DoorType::Red
                };

                let ptr_pair = game_data.reverse_door_ptr_pair_map[&(room_id, node_id)];
                let conn = map.doors.iter().find(|door_conn| {
                    door_conn.0 == ptr_pair || door_conn.1 == ptr_pair
                }).ok_or(anyhow!("Door connection not defined in map"))?;

                let locked_door = LockedDoor {
                    src_ptr_pair: conn.0,
                    dst_ptr_pair: conn.1,
                    door_type,
                    bidirectional: conn.2
                };
                
                *old_value = serde_json::to_value(&locked_door)?;
            }
        }

        let mut seed_data: SeedData = serde_json::from_value(v)?;

        // Upgrade map to include the room_mask
        if seed_data.map.room_mask.is_empty() {
            seed_data.map.room_mask = vec![true; seed_data.map.rooms.len()];
        }

        Ok(seed_data)
    }

    pub fn to_json(self) -> Result<Value> {
        let mut v: Value = serde_json::to_value(&self)?;

        // Compact room_mask in map down from a Vec<bool> to a Bitmask
        let map = v.get_mut("map").ok_or(anyhow!("Expected key: map"))?;
        let room_mask_v = map.get_mut("room_mask").ok_or(anyhow!("Expected key: room_mask"))?;
        let mut room_mask = vec![0u8; self.map.room_mask.len().div_ceil(8)];
        for (i, b) in self.map.room_mask.into_iter().enumerate() {
            if !b {
                continue;
            }
            let idx = i / 8;
            let offset = i % 8;
            room_mask[idx] |= 1 << offset;
        }
        *room_mask_v = serde_json::to_value(room_mask)?;

        // dst_ptr_pair and bidirectional are unused for door locks
        let door_locks = v.get_mut("door_locks").ok_or(anyhow!("Expected key: door_locks"))?;
        let door_locks_arr = door_locks.as_array_mut().ok_or(anyhow!("Expected key \"door_locks\" to be of type array"))?;
        for door_lock_v in door_locks_arr {
            let door_locks_obj = door_lock_v.as_object_mut().ok_or(anyhow!("Expected key \"door_locks\" to be of type object"))?;
            door_locks_obj.remove("dst_ptr_pair");
            door_locks_obj.remove("bidirectional");
        }

        // Tech and Notable settings can be reduced down to an array of enabled ids and (node_id, id)s
        let settings = v.get_mut("settings").ok_or(anyhow!("Expected key: settings"))?;
        let skill_assumption = settings.get_mut("skill_assumption_settings").ok_or(anyhow!("Expected key: skill_assumption_settings"))?;
        let skill_assumption_obj = skill_assumption.as_object_mut().ok_or(anyhow!("Expected key \"skill_assumption\" to be of type object"))?;

        let tech_reduced: Vec<i32> = self.settings.skill_assumption_settings.tech_settings.into_iter().filter_map(|tech| {
            if tech.enabled { Some(tech.id) } else { None }
        }).collect();
        let tech_reduced_v = serde_json::to_value(tech_reduced)?;
        let notable_reduced: Vec<(usize, usize)> = self.settings.skill_assumption_settings.notable_settings.into_iter().filter_map(|notable| {
            if notable.enabled { Some((notable.room_id, notable.notable_id)) } else { None }
        }).collect();
        let notable_reduced_v = serde_json::to_value(notable_reduced)?;

        skill_assumption_obj.insert("tech_settings".to_string(), tech_reduced_v);
        skill_assumption_obj.insert("notable_settings".to_string(), notable_reduced_v);

        // Item progression is mostly unused, using only ammo_collection_fraction and starting_items
        let item_progression = settings.get_mut("item_progression_settings").ok_or(anyhow!("Expected key: item_progression_settings"))?;
        let item_progression_obj = item_progression.as_object_mut().ok_or(anyhow!("Expected key \"item_progression_settings\" to be of type object"))?;
        item_progression_obj.retain(|k, _| {
            *k == "ammo_collection_fraction" || *k == "starting_items"
        });

        // Reduce Objective Settings to an array of enabled objective strings
        let objective_settings = settings.get_mut("objective_settings").ok_or(anyhow!("Expected key: objective_settings"))?;
        let objective_options = objective_settings.get_mut("objective_options").ok_or(anyhow!("Expected key: objective_options"))?;
        let objective_reduced: Vec<_> = self.settings.objective_settings.objective_options.into_iter().filter_map(|obj| {
            if obj.setting == ObjectiveSetting::Yes { Some(obj.objective) } else { None }
        }).collect();
        let objective_reduced_v = serde_json::to_value(objective_reduced)?;
        *objective_options = objective_reduced_v;

        Ok(v)
    }

    pub fn save_to_file(self, path: &Path) -> Result<()> {
        let v = self.to_json()?;
        let s = serde_json::to_string_pretty(&v)?;

        let mut f = std::fs::File::create(path)?;
        f.write_all(s.as_bytes())?;

        Ok(())
    }
}

fn get_key<'a>(v: &'a mut Value, k: &str) -> Result<&'a mut Value> {
    v.get_mut(k).ok_or(anyhow!("Expected key: \"{k}\""))
}

fn as_obj<'a>(v: &'a mut Value) -> Result<&'a mut serde_json::Map<String, Value>> {
    v.as_object_mut().ok_or(anyhow!("Expected value to be of type object"))
}

fn as_arr<'a>(v: &'a mut Value) -> Result<&'a mut Vec<Value>> {
    v.as_array_mut().ok_or(anyhow!("Expected value to be of type array"))
}