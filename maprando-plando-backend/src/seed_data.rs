use std::{io::{Read, Write}, path::Path};

use anyhow::{anyhow, bail, Result};
use hashbrown::HashMap;
use maprando::{preset::PresetData, randomize::LockedDoor, settings::{try_upgrade_settings, AreaAssignment, DoorLocksSize, DoorsMode, InitialMapRevealSettings, ItemCount, MapStationReveal, NotableSetting, ObjectiveScreen, ObjectiveSetting, ObjectiveSettings, OtherSettings, QualityOfLifeSettings, RandomizerSettings, SkillAssumptionSettings, StartLocationMode, StartLocationSettings, TechSetting, WallJump}};
use maprando_game::{BeamType, DoorType, GameData, Item, Map};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{Placeable, Plando, SpoilerOverride};

#[derive(Clone, Deserialize)]
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

    pub fn from_file(path: &Path, game_data: &GameData, preset_data: &PresetData) -> Result<SeedData> {
        if let Ok(res) = Self::try_from_json_legacy(path, game_data, preset_data) {
            return Ok(res);
        }

        let mut buf = Buffer::new(preset_data, Self::FORMAT.to_string());
        let mut f = std::fs::File::open(path)?;
        f.read_to_end(&mut buf.data)?;
        <SeedData as BufferSerializable>::deserialize(&mut buf)
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

    fn try_from_json_legacy(path: &Path, game_data: &GameData, preset_data: &PresetData) -> Result<SeedData> {
        let file = std::fs::read_to_string(path)?;

        let v = serde_json::from_str(&file)?;
        Self::from_json_legacy(v, game_data, preset_data)
    }

    fn from_json_legacy(mut v: Value, game_data: &GameData, preset_data: &PresetData) -> Result<SeedData> {
        if let Some(settings) = v.get_mut("settings") {
            if !settings.is_null() {
                let preset_string = settings.take().to_string();
                let preset_string = try_upgrade_settings(preset_string, preset_data, false)?.0;
                let preset: Value = serde_json::from_str(&preset_string)?;
                *settings = preset;
            }
        }
        // Start Location was stored as the vec idx in previous editions
        if let Some(start_loc_idx) = v.get_mut("start_location") {
            if let Some(idx) = start_loc_idx.as_u64() {
                let start_loc = if idx as usize >= game_data.start_locations.len() {
                    &Plando::get_ship_start()
                } else {
                    &game_data.start_locations[idx as usize]
                };
                *start_loc_idx = serde_json::to_value((start_loc.room_id, start_loc.node_id))?;
            }
        }
        // Locked doors were put in a serializable wrapper
        let map: Map = serde_json::from_value(v["map"].clone())?;
        if let Some(v_door_locks) = v.get_mut("door_locks") {
            if let Some(vec) = v_door_locks.as_array_mut() {
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
            }
        }

        let mut seed_data: SeedData = serde_json::from_value(v)?;

        // Upgrade map to include the room_mask
        if seed_data.map.room_mask.is_empty() {
            seed_data.map.room_mask = vec![true; seed_data.map.rooms.len()];
        }

        Ok(seed_data)
    }

    pub fn save_to_file(self, path: &Path, preset_data: &PresetData) -> Result<()> {
        let mut buf = Buffer::new(preset_data, Self::FORMAT.to_string());
        self.serialize(&mut buf);

        let mut f = std::fs::File::create(path)?;
        f.write_all(&buf.data)?;

        Ok(())
    }
}

struct Buffer<'g> {
    data: Vec<u8>,
    cursor: usize,
    preset_data: &'g PresetData,
    _version: String // Currently unused, can be used in later versions if format changes to ensure backwards compatibility
}

impl<'g> Buffer<'g> {
    fn new(preset_data: &'g PresetData, version: String) -> Self {
        Self {
            data: Vec::new(),
            cursor: 0,
            preset_data,
            _version: version
        }
    }

    fn write<T: BufferSerializable>(&mut self, v: T) {
        v.serialize(self);
    }

    fn write_bool(&mut self, v: bool) {
        self.write_byte(v as u8);
    }

    fn write_byte(&mut self, v: u8) {
        self.data.push(v);
    }

    fn write_bytes(&mut self, mut v: Vec<u8>) {
        self.data.append(&mut v);
    }

    fn write_enum<E: Serialize>(&mut self, v: E) {
        let str = serde_json::to_string(&v).unwrap();
        self.write(str);
    }

    fn read<T: BufferSerializable>(&mut self) -> Result<T> {
        T::deserialize(self)
    }

    fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_byte()? == 1)
    }

    fn read_byte(&mut self) -> Result<u8> {
        let v = self.data.get(self.cursor).ok_or(anyhow!("Unexpected EOF"))?;
        self.cursor += 1;
        Ok(*v)
    }

    fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        if self.cursor + N > self.data.len() {
            bail!("Unexpected EOF");
        }

        let mut buf = [0u8; N];
        let slice = &self.data[self.cursor..self.cursor + N];
        buf.copy_from_slice(slice);
        self.cursor += N;
        Ok(buf)
    }

    fn read_slice(&mut self, len: usize) -> Result<&[u8]> {
        if self.cursor + len > self.data.len() {
            bail!("Unexpected EOF");
        }

        let slice = &self.data[self.cursor..self.cursor + len];
        self.cursor += len;
        Ok(slice)
    }

    fn read_enum<E: DeserializeOwned>(&mut self) -> Result<E> {
        let str = self.read::<String>()?;
        Ok(serde_json::from_str(&str)?)
    }
}

trait BufferSerializable where Self: Sized {
    fn serialize(self, buf: &mut Buffer);
    fn deserialize(buf: &mut Buffer) -> Result<Self>;
}

impl BufferSerializable for u8 {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        buf.read_byte()
    }

    fn serialize(self, buf: &mut Buffer) {
        buf.write_byte(self);
    }
}

impl BufferSerializable for u32 {
    fn serialize(self, buf: &mut Buffer) {
        let v = self.to_be_bytes().to_vec();
        buf.write_bytes(v);
    }

    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        const SIZE: usize = size_of::<u32>();
        let slice = buf.read_bytes::<SIZE>()?;
        Ok(Self::from_be_bytes(slice))
    }
}

impl BufferSerializable for usize {
    fn serialize(self, buf: &mut Buffer) {
        (self as u32).serialize(buf);
    }

    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        <u32 as BufferSerializable>::deserialize(buf).map(|res| res as usize)
    }
}

impl BufferSerializable for i32 {
    fn serialize(self, buf: &mut Buffer) {
        let v = self.to_be_bytes().to_vec();
        buf.write_bytes(v);
    }

    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        const SIZE: usize = size_of::<i32>();
        let slice = buf.read_bytes::<SIZE>()?;
        Ok(Self::from_be_bytes(slice))
    }
}

impl BufferSerializable for f32 {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        const SIZE: usize = size_of::<f32>();
        let slice = buf.read_bytes::<SIZE>()?;
        Ok(Self::from_be_bytes(slice))
    }

    fn serialize(self, buf: &mut Buffer) {
        let v = self.to_be_bytes().to_vec();
        buf.write_bytes(v);
    }
}

impl BufferSerializable for String {
    fn serialize(self, buf: &mut Buffer) {
        buf.write(self.len());
        buf.write_bytes(self.as_bytes().to_vec());
    }

    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let size = buf.read()?;
        let slice = buf.read_slice(size)?.to_vec();
        Ok(Self::from_utf8(slice)?)
    }
}

impl<T: BufferSerializable> BufferSerializable for Option<T> {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let is_some = buf.read_byte()? == 1;
        Ok(if is_some {
            Some(buf.read()?)
        } else {
            None
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        buf.write_byte(self.is_some() as u8);
        if let Some(v) = self {
            buf.write(v);
        }
    }
}

impl<T: BufferSerializable> BufferSerializable for Vec<T> {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let len: usize = buf.read()?;
        let mut res = vec![];
        for _ in 0..len {
            res.push(buf.read()?);
        }
        Ok(res)
    }

    fn serialize(self, buf: &mut Buffer) {
        buf.write(self.len());
        for elem in self {
            buf.write(elem);
        }
    }
}

impl<T: BufferSerializable, U: BufferSerializable> BufferSerializable for (T, U) {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        Ok((buf.read()?, buf.read()?))
    }

    fn serialize(self, buf: &mut Buffer) {
        buf.write(self.0);
        buf.write(self.1);
    }
}

impl BufferSerializable for Vec<bool> {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let size: usize = buf.read()?;
        let slice = buf.read_slice(size.div_ceil(8))?;
        let mut res = vec![false; size];

        for i in 0..slice.len() {
            for bit in 0..8 {
                if (slice[i] & (1 << bit)) != 0 {
                    res[i * 8 + bit] = true;
                }
            }
        }

        Ok(res)
    }

    fn serialize(self, buf: &mut Buffer) {
        let mut res = vec![0u8; self.len().div_ceil(8)];

        for (idx, elem) in self.iter().enumerate() {
            if !*elem {
                continue;
            }
            let res_idx = idx / 8;
            let offset = idx % 8;
            res[res_idx] |= 1u8 << offset;
        }

        buf.write(self.len());
        buf.write_bytes(res);
    }
}

impl BufferSerializable for SeedData {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let version: String = buf.read()?;

        if version != Self::FORMAT {
            bail!("Invalid Version String: Format not supported");
        }

        let map: Map = buf.read()?;
        let start_location = buf.read()?;
        let item_placements: Vec<Item> = buf.read::<Vec<u8>>()?.into_iter().map(|item_idx| {
            Item::try_from(item_idx as usize).unwrap_or(Item::Nothing)
        }).collect();

        let num_door_locks = buf.read()?;
        let mut door_locks = Vec::with_capacity(num_door_locks);
        for _ in 0..num_door_locks {
            let src_ptr_pair = buf.read()?;
            let door_type = buf.read_enum()?;
            let dst_ptr_pair = if door_type == DoorType::Wall {
                (None, None)
            } else {
                map.doors.iter().find_map(|(src, dst, _)| {
                    if *src == src_ptr_pair {
                        Some(*dst)
                    } else if *dst == src_ptr_pair {
                        Some(*src)
                    } else {
                        None
                    }
                }).unwrap_or((None, None))
            };
            door_locks.push(LockedDoor {
                src_ptr_pair,
                dst_ptr_pair,
                door_type,
                bidirectional: door_type != DoorType::Wall
            });
        }

        let creator_name = buf.read()?;
        let custom_escape_time = buf.read()?;

        let num_overrides = buf.read()?;
        let mut spoiler_overrides = Vec::with_capacity(num_overrides);
        for _ in 0..num_overrides {
            let item_idx = buf.read()?;
            let step = buf.read()?;
            let description = buf.read()?;
            spoiler_overrides.push(SpoilerOverride {
                item_idx, step, description
            });
        }

        let settings: RandomizerSettings = buf.read()?;

        Ok(Self {
            map,
            start_location,
            item_placements,
            door_locks,
            settings,
            spoiler_overrides,
            custom_escape_time,
            creator_name
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        let version = Self::FORMAT.to_string();

        // Write version number
        buf.write(version);

        // Write Map
        buf.write(self.map);

        // Write Start location
        buf.write(self.start_location);

        // Write items
        buf.write::<Vec<u8>>(self.item_placements.iter().map(|item| *item as u8).collect());

        // Write door locks, only src_ptr_pair and door_type is stored as the rest can be induced:
        // dst_ptr_pair is the connecting room, bidirection is only false for wall doors (in which case dst_ptr_pair is None)
        buf.write(self.door_locks.len());
        for door_lock in &self.door_locks {
            buf.write(door_lock.src_ptr_pair);
            buf.write_enum(door_lock.door_type);
        }

        // Write creator name
        buf.write(self.creator_name.clone());

        // Write custom escape timer
        buf.write(self.custom_escape_time);

        // Write spoiler overrides
        buf.write(self.spoiler_overrides.len());
        for so in &self.spoiler_overrides {
            buf.write(so.item_idx);
            buf.write(so.step);
            buf.write(so.description.clone());
        }

        // Write Logic Settings
        buf.write(self.settings.clone());
    }
}

impl BufferSerializable for Map {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let room_mask = buf.read()?;
        
        let rooms: Vec<(usize, usize)> = buf.read()?;

        let mut doors = vec![];
        let doors_len: usize = buf.read()?;
        for _ in 0..doors_len {
            let src = buf.read()?;
            let dst = buf.read()?;
            let bi = buf.read_bool()?;
            doors.push((src, dst, bi));
        }

        let area = buf.read_slice(rooms.len())?.iter().map(|v| *v as usize).collect();
        let subarea = buf.read::<Vec<bool>>()?.into_iter().map(|v| v as usize).collect();
        let subsubarea = buf.read::<Vec<bool>>()?.into_iter().map(|v| v as usize).collect();

        Ok(Self {
            room_mask,
            rooms,
            doors,
            area,
            subarea,
            subsubarea
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        // Write Room Mask
        buf.write(self.room_mask);

        // Write Room Positions (Vec of (usize, usize) pairs)
        buf.write(self.rooms);

        // Write Door Connections (Vec of ((usize, usize), (usize, usize), bool))
        buf.write(self.doors.len());
        for (src, dst, bi) in &self.doors {
            buf.write(*src);
            buf.write(*dst);
            buf.write_bool(*bi);
        }

        // Write area vec
        buf.write_bytes(self.area.iter().map(|x| *x as u8).collect());
        // Write subarea vec
        buf.write(self.subarea.iter().map(|x| *x == 1).collect::<Vec<bool>>());
        // Write subsubarea vec
        buf.write(self.subsubarea.iter().map(|x| *x == 1).collect::<Vec<bool>>());
    }
}

impl BufferSerializable for RandomizerSettings {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let base_preset = &buf.preset_data.default_preset;

        let version = buf.read()?;
        let name = buf.read()?;
        let skill_assumption_settings = buf.read()?;

        let mut item_progression_settings = base_preset.item_progression_settings.clone();
        item_progression_settings.starting_items = buf.read()?;

        let quality_of_life_settings = buf.read()?;

        let objective_settings = buf.read()?;

        let save_animals = buf.read_enum()?;
        let other_settings = buf.read()?;
        let mut res = Self {
            version,
            name,
            skill_assumption_settings,
            item_progression_settings,
            quality_of_life_settings,
            objective_settings,
            map_layout: "".to_string(),
            doors_mode: DoorsMode::Beam,
            start_location_settings: StartLocationSettings {
                mode: StartLocationMode::Ship,
                room_id: None,
                node_id: None
            },
            save_animals,
            other_settings,
            debug: false
        };

        // Match any potential presets
        res.skill_assumption_settings.preset = buf.preset_data.skill_presets.iter().find_map(|preset| {
            if res.skill_assumption_settings == *preset { preset.preset.clone() } else { None }
        });
        res.quality_of_life_settings.preset = buf.preset_data.quality_of_life_presets.iter().find_map(|preset| {
            if res.quality_of_life_settings == *preset { preset.preset.clone() } else { None }
        });
        res.objective_settings.preset = buf.preset_data.objective_presets.iter().find_map(|preset| {
            if res.objective_settings == *preset { preset.preset.clone() } else { None }
        });

        Ok(res)
    }

    fn serialize(self, buf: &mut Buffer) {
        buf.write(self.version);
        buf.write(self.name);
        buf.write(self.skill_assumption_settings);
        buf.write(self.item_progression_settings.starting_items);
        buf.write(self.quality_of_life_settings);
        buf.write(self.objective_settings);
        //buf.write(self.map_layout); Map layout not needed, saved as Creator Name
        //buf.write(self.doors_mode); Determined while loading a seed
        //buf.write(self.start_location_settings); Saved separately
        buf.write_enum(self.save_animals);
        buf.write(self.other_settings);
        //buf.write(self.debug); Unused
    }
}

impl BufferSerializable for ItemCount {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let item_idx: usize = buf.read()?;
        let count = buf.read()?;
        Ok(Self {
            item: Item::try_from(item_idx)?,
            count
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        buf.write(self.item as usize);
        buf.write(self.count);
    }
}

impl BufferSerializable for SkillAssumptionSettings {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        //let preset = buf.read(); Ignore presets
        let shinespark_tiles = buf.read()?;
        let heated_shinespark_tiles = buf.read()?;
        let speed_ball_tiles = buf.read()?;
        let shinecharge_leniency_frames = buf.read()?;
        let resource_multiplier = buf.read()?;
        let farm_time_limit = buf.read()?;
        let gate_glitch_leniency = buf.read()?;
        let door_stuck_leniency = buf.read()?;
        let bomb_into_cf_leniency = buf.read()?;
        let jump_into_cf_leniency = buf.read()?;
        let spike_xmode_leniency = buf.read()?;
        let phantoon_proficiency = buf.read()?;
        let draygon_proficiency = buf.read()?;
        let ridley_proficiency = buf.read()?;
        let botwoon_proficiency = buf.read()?;
        let mother_brain_proficiency = buf.read()?;
        let escape_timer_multiplier = buf.read()?;

        let tech_ids: Vec<i32> = buf.read()?;
        let notable_ids: Vec<(usize, usize)> = buf.read()?;

        let base_preset = &buf.preset_data.default_preset.skill_assumption_settings;

        let mut tech_settings: HashMap<i32, TechSetting> = base_preset.tech_settings.iter().map(|tech| {
            (tech.id, tech.clone())
        }).collect();
        let mut notable_settings: HashMap<(usize, usize), NotableSetting> = base_preset.notable_settings.iter().map(|info| {
            ((info.room_id, info.notable_id), info.clone())
        }).collect();

        for tech_id in tech_ids {
            if let Some(elem) = tech_settings.get_mut(&tech_id) {
                elem.enabled = true;
            } else {
                println!("WARN: Unknown tech ID {tech_id}");
            }
        }
        for (room_id, notable_id) in notable_ids {
            if let Some(elem) = notable_settings.get_mut(&(room_id, notable_id)) {
                elem.enabled = true;
            } else {
                println!("WARN: Unknown notable ID {notable_id}");
            }
        }

        Ok(Self {
            preset: None,
            shinespark_tiles,
            heated_shinespark_tiles,
            speed_ball_tiles,
            shinecharge_leniency_frames,
            resource_multiplier,
            farm_time_limit,
            gate_glitch_leniency,
            door_stuck_leniency,
            bomb_into_cf_leniency,
            jump_into_cf_leniency,
            spike_xmode_leniency,
            phantoon_proficiency,
            draygon_proficiency,
            ridley_proficiency,
            botwoon_proficiency,
            mother_brain_proficiency,
            escape_timer_multiplier,
            tech_settings: tech_settings.into_values().collect(),
            notable_settings: notable_settings.into_values().collect()
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        //buf.write(self.preset.unwrap_or_default()); Ignore presets
        buf.write(self.shinespark_tiles);
        buf.write(self.heated_shinespark_tiles);
        buf.write(self.speed_ball_tiles);
        buf.write(self.shinecharge_leniency_frames);
        buf.write(self.resource_multiplier);
        buf.write(self.farm_time_limit);
        buf.write(self.gate_glitch_leniency);
        buf.write(self.door_stuck_leniency);
        buf.write(self.bomb_into_cf_leniency);
        buf.write(self.jump_into_cf_leniency);
        buf.write(self.spike_xmode_leniency);
        buf.write(self.phantoon_proficiency);
        buf.write(self.draygon_proficiency);
        buf.write(self.ridley_proficiency);
        buf.write(self.botwoon_proficiency);
        buf.write(self.mother_brain_proficiency);
        buf.write(self.escape_timer_multiplier);

        let tech_enabled: Vec<i32> = self.tech_settings.iter().filter_map(|tech| {
            if tech.enabled { Some(tech.id) } else { None }
        }).collect();
        buf.write(tech_enabled);

        let notable_enabled: Vec<(usize, usize)> = self.notable_settings.iter().filter_map(|notable| {
            if notable.enabled { Some((notable.room_id, notable.notable_id)) } else { None }
        }).collect();
        buf.write(notable_enabled);
    }
}

impl BufferSerializable for QualityOfLifeSettings {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let initial_map_reveal_settings = buf.read()?;
        let item_markers = buf.read_enum()?;
        let room_outline_revealed = buf.read_bool()?;
        let opposite_area_revealed = buf.read_bool()?;
        let mother_brain_fight = buf.read_enum()?;
        let supers_double = buf.read_bool()?;
        let escape_movement_items = buf.read_bool()?;
        let escape_refill = buf.read_bool()?;
        let escape_enemies_cleared = buf.read_bool()?;
        let fast_elevators = buf.read_bool()?;
        let fast_doors = buf.read_bool()?;
        let fast_pause_menu = buf.read_bool()?;
        let fanfares = buf.read_enum()?;
        let respin = buf.read_bool()?;
        let infinite_space_jump = buf.read_bool()?;
        let momentum_conservation = buf.read_bool()?;
        let all_items_spawn = buf.read_bool()?;
        let acid_chozo = buf.read_bool()?;
        let remove_climb_lava = buf.read_bool()?;
        let etank_refill = buf.read_enum()?;
        let energy_station_reserves = buf.read_bool()?;
        let disableable_etanks = buf.read_bool()?;
        let reserve_backward_transfer = buf.read_bool()?;
        let buffed_drops = buf.read_bool()?;
        Ok(Self {
            preset: None,
            initial_map_reveal_settings,
            item_markers,
            room_outline_revealed,
            opposite_area_revealed,
            mother_brain_fight,
            supers_double,
            escape_movement_items,
            escape_refill,
            escape_enemies_cleared,
            fast_elevators,
            fast_doors,
            fast_pause_menu,
            fanfares,
            respin,
            infinite_space_jump,
            momentum_conservation,
            all_items_spawn,
            acid_chozo,
            remove_climb_lava,
            etank_refill,
            energy_station_reserves,
            disableable_etanks,
            reserve_backward_transfer,
            buffed_drops,
            early_save: true
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        //buf.write(self.preset); Preset not needed
        buf.write(self.initial_map_reveal_settings);
        buf.write_enum(self.item_markers);
        buf.write_bool(self.room_outline_revealed);
        buf.write_bool(self.opposite_area_revealed);
        buf.write_enum(self.mother_brain_fight);
        buf.write_bool(self.supers_double);
        buf.write_bool(self.escape_movement_items);
        buf.write_bool(self.escape_refill);
        buf.write_bool(self.escape_enemies_cleared);
        buf.write_bool(self.fast_elevators);
        buf.write_bool(self.fast_doors);
        buf.write_bool(self.fast_pause_menu);
        buf.write_enum(self.fanfares);
        buf.write_bool(self.respin);
        buf.write_bool(self.infinite_space_jump);
        buf.write_bool(self.momentum_conservation);
        buf.write_bool(self.all_items_spawn);
        buf.write_bool(self.acid_chozo);
        buf.write_bool(self.remove_climb_lava);
        buf.write_enum(self.etank_refill);
        buf.write_bool(self.energy_station_reserves);
        buf.write_bool(self.disableable_etanks);
        buf.write_bool(self.reserve_backward_transfer);
        buf.write_bool(self.buffed_drops);
        //buf.write(self.early_save);
    }
}

impl BufferSerializable for InitialMapRevealSettings {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let map_stations = buf.read_enum()?;
        let save_stations = buf.read_enum()?;
        let refill_stations = buf.read_enum()?;
        let ship = buf.read_enum()?;
        let objectives = buf.read_enum()?;
        let area_transitions = buf.read_enum()?;
        let items1 = buf.read_enum()?;
        let items2 = buf.read_enum()?;
        let items3 = buf.read_enum()?;
        let items4 = buf.read_enum()?;
        let other = buf.read_enum()?;
        let all_areas = buf.read_bool()?;
        Ok(Self {
            preset: None,
            map_stations, save_stations, refill_stations, ship, objectives,
            area_transitions, items1, items2, items3, items4, other, all_areas
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        buf.write_enum(self.map_stations);
        buf.write_enum(self.save_stations);
        buf.write_enum(self.refill_stations);
        buf.write_enum(self.ship);
        buf.write_enum(self.objectives);
        buf.write_enum(self.area_transitions);
        buf.write_enum(self.items1);
        buf.write_enum(self.items2);
        buf.write_enum(self.items3);
        buf.write_enum(self.items4);
        buf.write_enum(self.other);
        buf.write_bool(self.all_areas);
    }
}

impl BufferSerializable for ObjectiveSettings {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let mut res = buf.preset_data.objective_presets[0].clone();

        res.preset = None;

        let objectives: Vec<bool> = buf.read()?;
        let num_obj = objectives.iter().filter(|x| **x).count();
        for idx in 0..objectives.len() {
            res.objective_options[idx].setting = if objectives[idx] {
                ObjectiveSetting::Yes
            } else {
                ObjectiveSetting::No
            };
        }
        res.min_objectives = num_obj as i32;
        res.max_objectives = num_obj as i32;
        res.objective_screen = if buf.read_bool()? {
            ObjectiveScreen::Enabled
        } else {
            ObjectiveScreen::Disabled
        };

        Ok(res)
    }

    fn serialize(self, buf: &mut Buffer) {
        let objectives: Vec<_> = self.objective_options.into_iter().map(|opt| {
            opt.setting == ObjectiveSetting::Yes
        }).collect();

        buf.write(objectives);
        buf.write_bool(self.objective_screen == ObjectiveScreen::Enabled);
    }
}

impl BufferSerializable for OtherSettings {
    fn deserialize(buf: &mut Buffer) -> Result<Self> {
        let vec: Vec<bool> = buf.read()?;
        let wall_jump = if vec[0] { WallJump::Collectible } else { WallJump::Vanilla };
        let door_locks_size = if vec[1] { DoorLocksSize::Large } else { DoorLocksSize::Small };
        let map_station_reveal = if vec[2] { MapStationReveal::Full } else { MapStationReveal::Partial };
        let energy_free_shinesparks = vec[3];
        let ultra_low_qol = vec[4];
        Ok(Self {
            wall_jump,
            area_assignment: AreaAssignment::Random,
            door_locks_size,
            map_station_reveal,
            energy_free_shinesparks,
            ultra_low_qol,
            race_mode: false,
            random_seed: None
        })
    }

    fn serialize(self, buf: &mut Buffer) {
        let wall_jump = self.wall_jump == WallJump::Collectible;
        let door_locks_size = self.door_locks_size == DoorLocksSize::Large;
        let map_station_reveal = self.map_station_reveal == MapStationReveal::Full;
        let energy_free_shinesparks = self.energy_free_shinesparks;
        let ultra_low_qol = self.ultra_low_qol;
        let vec = vec![wall_jump, door_locks_size, map_station_reveal, energy_free_shinesparks, ultra_low_qol];
        buf.write(vec);
    }
}

