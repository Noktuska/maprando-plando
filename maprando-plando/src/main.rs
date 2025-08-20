use crate::{backend::{map_editor::{self, MapEditor, MapErrorType}, plando::{get_double_item_offset, DoubleItemPlacement, MapRepositoryType, Placeable, Plando, SpoilerOverride, ITEM_VALUES}}, input_state::KeyState, layout::{hotkey_settings::Keybind, map_editor_ui::MapEditorUi, room_search::{RoomSearch, SearchOpt}, settings_customize::{Customization, SettingsCustomize, SettingsCustomizeResult}, settings_logic::LogicCustomization, Layout, SidebarPanel, WindowType}};
use anyhow::{anyhow, bail, Result};
use egui::{self, style::default_text_styles, Color32, Context, FontDefinitions, Id, RichText, Sense, TextureId, Ui, Vec2};
use egui_sfml::{SfEgui, UserTexSource};
use hashbrown::{HashMap, HashSet};
use input_state::MouseState;
use maprando::{patch::Rom, preset::PresetData, randomize::{LockedDoor, SpoilerRouteEntry}, settings::{DoorsMode, Objective, RandomizerSettings}};
use maprando_game::{BeamType, DoorType, GameData, Item, Map, MapTileEdge, MapTileInterior, MapTileSpecialType};
use rand::RngCore;
use rfd::FileDialog;
use self_update::cargo_crate_version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sfml::{
        cpp::FBox, graphics::{
            self, Color, FloatRect, IntRect, PrimitiveType, RenderStates, RenderTarget, RenderWindow, Shape, Transformable, Vertex
        }, system::{Vector2f, Vector2i}, window::{
            mouse, ContextSettings, Event, Key, Style
        }
    };
use strum::VariantArray;
use strum_macros::VariantArray;
use std::{cmp::{max, min}, ffi::OsStr, fs::File, io::{Read, Write}, path::Path, thread::{self, JoinHandle}, time::Instant};

mod backend;
mod layout;
mod input_state;
mod utils;
mod egui_sfml;

#[derive(Clone)]
struct RoomData {
    room_id: usize,
    room_idx: usize,
    tile_width: u32,
    tile_height: u32,
    atlas_x_offset: u32,
    atlas_y_offset: u32
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    mouse_click_pos_tolerance: i32,
    mouse_click_delay_tolerance: i32,
    rom_path: String,
    spoiler_auto_update: bool,
    last_customization: Customization,
    last_logic_preset: Option<RandomizerSettings>,
    disable_logic: bool,
    auto_update: bool,
    disable_bg_grid: bool,
    ui_scale: f32,
    scroll_speed: f32,
    hotkeys: HashMap<usize, Vec<Key>>,
    fps_cap: u32,
    animation_speed: f32,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            mouse_click_pos_tolerance: 5,
            mouse_click_delay_tolerance: 60,
            rom_path: String::new(),
            spoiler_auto_update: true,
            last_customization: Customization::default(),
            last_logic_preset: None,
            disable_logic: false,
            auto_update: true,
            disable_bg_grid: false,
            ui_scale: 1.0,
            scroll_speed: 16.0,
            hotkeys: HashMap::new(),
            fps_cap: 60,
            animation_speed: 1.0
        }
    }
}

fn load_vanilla_rom(rom_path: &Path) -> Result<Rom> {
    let rom_data = std::fs::read(rom_path)?;
    let rom = Rom::new(rom_data);

    if rom.data.len() == 0 {
        bail!("ROM data is empty");
    }
    let rom_digest = crypto_hash::hex_digest(crypto_hash::Algorithm::SHA256, &rom.data);
    if rom_digest != "12b77c4bc9c1832cee8881244659065ee1d84c70c3d29e6eaf92e6798cc2ca72" {
        bail!("Invalid ROM data");
    }
    Ok(rom)
}

#[derive(Serialize, Deserialize)]
struct SeedData {
    map: Map,
    start_location: (usize, usize), // RoomId, NodeId
    item_placements: Vec<Item>,
    door_locks: Vec<LockedDoor>,
    settings: RandomizerSettings,
    #[serde(default = "Vec::new")]
    spoiler_overrides: Vec<SpoilerOverride>
}

impl SeedData {
    fn new(plando: &Plando) -> Self {
        let start_loc = &plando.start_location_data.start_location;

        SeedData {
            map: plando.map().clone(),
            start_location: (start_loc.room_id, start_loc.node_id),
            item_placements: plando.item_locations.clone(),
            door_locks: plando.locked_doors.clone(),
            settings: plando.randomizer_settings.clone(),
            spoiler_overrides: plando.spoiler_overrides.clone()
        }
    }
}

fn save_settings(settings: &Settings, path: &Path) -> Result<()> {
    let mut file = File::create(path)?;
    let data = serde_json::to_string_pretty(&settings)?;
    file.write_all(data.as_bytes())?;
    Ok(())
}

fn load_settings(path: &Path, preset_data: &PresetData) -> Result<Settings> {
    let mut file = File::open(path)?;
    let mut data_str = String::new();
    file.read_to_string(&mut data_str)?;

    let mut v: Value = serde_json::from_str(&data_str)?;

    let mut preset_opt = None;
    if let Some(last_logic_preset) = v.get_mut("last_logic_preset") {
        if !last_logic_preset.is_null() {
            let preset_string = last_logic_preset.take().to_string();
            preset_opt = Some(utils::try_upgrade_settings(preset_string, preset_data, true)?.1);
        }
    }

    let mut result: Settings = serde_json::from_value(v)?;
    result.last_logic_preset = preset_opt;

    Ok(result)
}

fn load_seed(plando: &mut Plando, path: &Path, preset_data: &PresetData) -> Result<()> {
    let mut file = File::open(path)?;

    let mut str = String::new();
    file.read_to_string(&mut str)?;

    // Keep seed data somewhat backwards compatible
    // Upgrade randomizer settings
    let mut v: Value = serde_json::from_str(&str)?;
    if let Some(settings) = v.get_mut("settings") {
        if !settings.is_null() {
            let preset_string = settings.take().to_string();
            let preset_string = utils::try_upgrade_settings(preset_string, preset_data, true)?.0;
            let preset: Value = serde_json::from_str(&preset_string)?;
            *settings = preset;
        }
    }
    // Start Location was stored as the vec idx in previous editions
    if let Some(start_loc_idx) = v.get_mut("start_location") {
        if let Some(idx) = start_loc_idx.as_u64() {
            let start_loc = &plando.game_data.start_locations[idx as usize];
            *start_loc_idx = serde_json::to_value((start_loc.room_id, start_loc.node_id))?;
        }
    }
    // Locked doors were put in a serializable wrapper. This is not really upgradeable without huge overhead so we clear all the doors
    if let Some(v) = v.get_mut("door_locks") {
        if let Some(vec) = v.as_array() {
            if !vec.is_empty() {
                if serde_json::from_value::<LockedDoor>(vec[0].clone()).is_err() {
                    *v = Value::Array(Vec::new());
                }
            }
        }
    }

    let mut seed_data: SeedData = serde_json::from_value(v)?;

    // Upgrade map to include the room_mask
    if seed_data.map.room_mask.is_empty() {
        seed_data.map.room_mask = vec![true; seed_data.map.rooms.len()];
    }

    plando.load_preset(seed_data.settings);

    plando.load_map(seed_data.map)?;

    plando.item_locations = seed_data.item_placements;
    for item in &plando.item_locations {
        if *item != Item::Nothing {
            plando.placed_item_count[*item as usize + Placeable::ETank as usize] += 1;
        }
    }
    
    for door_data in seed_data.door_locks {
        let (room_idx, door_idx) = plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door_data.src_ptr_pair];

        plando.place_door(room_idx, door_idx, Some(door_data.door_type), false, true)?;
    }

    let start_loc_idx = plando.game_data.start_location_id_map[&seed_data.start_location];
    plando.place_start_location(plando.game_data.start_locations[start_loc_idx].clone())?;

    plando.spoiler_overrides = seed_data.spoiler_overrides;

    plando.update_spoiler_data()?;

    Ok(())
}

fn save_seed(plando: &mut Plando, path: &Path) -> Result<()> {
    let mut file = File::create(path)?;

    let seed_data = SeedData::new(plando);

    let out = serde_json::to_string(&seed_data)?;

    file.write_all(out.as_bytes())?;

    Ok(())
}

fn load_preset_data(game_data: &GameData) -> Result<PresetData> {
    let tech_path = Path::new("./data/tech_data.json");
    let notable_path = Path::new("./data/notable_data.json");
    let presets_path = Path::new("./data/presets/");

    let preset_data = PresetData::load(tech_path, notable_path, presets_path, game_data);
    preset_data
}

fn download_map_repos() -> Result<()> {
    for pool in ["v119-standard-avro", "v119-wild-avro"] {
        let url = format!("https://map-rando-artifacts.s3.us-west-004.backblazeb2.com/maps/{pool}.tar");
        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send()?;

        println!("Attempting to download {url}");
        let bytes = resp.bytes()?;
        let cursor = std::io::Cursor::new(&bytes);
        //let decoder = GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(cursor);
        println!("Unpacking archive, this may take a while...");
        archive.unpack("../maps/")?;
    }
    println!("Done");
    Ok(())
}

fn check_update() -> Result<()> {
    let cur_ver = cargo_crate_version!();

    println!("Checking for updates...");
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner("Noktuska")
        .repo_name("maprando-plando")
        .build()?
        .fetch()?;

    if releases.is_empty() {
        bail!("No releases found");
    }
    let release = releases[0].clone();
    println!("Found release {} ({} total releases)", release.version, releases.len());

    if !self_update::version::bump_is_greater(&cur_ver, &release.version)? {
        bail!("Current release is up to date");
    }
    println!("Found update {} --> {}", &cur_ver, &release.version);
    loop {
        println!("Do you want to update? (Y/N) ");
        std::io::stdout().flush()?;
        let mut s = String::new();
        match std::io::stdin().read_line(&mut s) {
            Err(err) => bail!(err.to_string()),
            Ok(_) => match s.to_lowercase().trim() {
                "y" => break,
                "n" => bail!("Update declined"),
                _ => {}
            }
        }
    }

    let asset = release.assets.first().ok_or(anyhow!("Could not find downloadable asset"))?;

    let tmp_dir_path = Path::new("./tmp/");
    let tmp_archive_path = tmp_dir_path.join(&asset.name);
    let file_ext = tmp_archive_path.extension().ok_or(anyhow!("Found asset has no file extension"))?.to_str().unwrap();

    std::fs::create_dir_all(tmp_dir_path)?;
    let tmp_file = File::create(&tmp_archive_path)?;

    println!("Downloading...");
    self_update::Download::from_url(&asset.download_url)
        .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
        .download_to(tmp_file)?;

    match file_ext {
        "exe" => {
            println!("Replacing executable...");
            self_update::self_replace::self_replace(&tmp_archive_path)?;
        }
        "zip" => {
            let exe_path = std::env::current_exe()?;
            let dir_path = exe_path.parent().unwrap();
            
            let file = File::open(&tmp_archive_path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let new_exe_path = dir_path.join(Path::new("maprando-plando__new__.exe"));

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let path = match file.enclosed_name() {
                    Some(path) => path,
                    None => continue
                };

                let out_path = dir_path.join(&path);
                if file.is_dir() {
                    std::fs::create_dir_all(out_path)?;
                } else if path.file_name() == Some(OsStr::new("maprando-plando.exe")) {
                    let mut outfile = File::create(&new_exe_path)?;
                    std::io::copy(&mut file, &mut outfile)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }
                    let mut outfile = File::create(out_path)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }

            self_update::self_replace::self_replace(&new_exe_path)?;
        }
        _ => bail!("Unexpected file type")
    }

    println!("Removing tmp directory...");
    std::fs::remove_dir_all(tmp_dir_path)?;

    println!("Done!");
    Ok(())
}

enum SpecialRoom {
    EnergyRefill,
    AmmoRefill,
    FullRefill,
    SaveStation,
    MapStation,
    Objective
}

fn get_special_room_mask(room_type: SpecialRoom) -> [[u8; 8]; 8] {
    match room_type {
        SpecialRoom::EnergyRefill =>
            [[1, 1, 1, 1, 1, 1, 1, 1],
            [1, 1, 1, 0, 0, 1, 1, 1],
            [1, 1, 1, 0, 0, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 1, 1, 0, 0, 1, 1, 1],
            [1, 1, 1, 0, 0, 1, 1, 1],
            [1, 1, 1, 1, 1, 1, 1, 1]],
        SpecialRoom::AmmoRefill => 
            [[1, 1, 1, 1, 1, 1, 1, 1],
            [1, 1, 1, 0, 0, 1, 1, 1],
            [1, 1, 0, 0, 0, 0, 1, 1],
            [1, 1, 0, 1, 1, 0, 1, 1],
            [1, 1, 0, 0, 0, 0, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 1, 0, 0, 1, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1]],
        SpecialRoom::FullRefill =>
            [[1, 1, 1, 1, 1, 1, 1, 1],
            [1, 0, 1, 0, 0, 1, 0, 1],
            [1, 1, 1, 0, 0, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 1, 1, 0, 0, 1, 1, 1],
            [1, 0, 1, 0, 0, 1, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1]],
        SpecialRoom::SaveStation =>
            [[1, 1, 1, 1, 1, 1, 1, 1],
            [1, 1, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 1, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 1, 1],
            [1, 1, 0, 0, 0, 0, 0, 1],
            [1, 1, 1, 1, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 1, 1],
            [1, 1, 1, 1, 1, 1, 1, 1]],
        SpecialRoom::MapStation =>
            [[1, 1, 1, 1, 1, 1, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 1, 1, 1, 1, 0, 1],
            [1, 0, 1, 0, 0, 1, 0, 1],
            [1, 0, 1, 0, 0, 1, 0, 1],
            [1, 0, 1, 1, 1, 1, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1]],
        SpecialRoom::Objective =>
            [[1, 1, 1, 1, 1, 1, 1, 1],
            [1, 0, 0, 1, 1, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 1, 0, 0, 0, 0, 1, 1],
            [1, 1, 0, 0, 0, 0, 1, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 1, 1, 0, 0, 1],
            [1, 1, 1, 1, 1, 1, 1, 1]]
    }
}

fn get_objective_mask(room_id: usize, tile_x: usize, tile_y: usize) -> bool {
    match room_id {
        219 => !(tile_x == 1 && tile_y == 2), // Plasma Room
        161 => tile_x == 4 && tile_y == 1, // Bowling
        149 => tile_x == 0 && tile_y == 0, // Acid Chozo
        150 => tile_y == 1, // GT
        _ => true
    }
}

fn get_explored_color(value: u8, area: usize) -> graphics::Color {
    let cool_area_color = match area {
        0 => graphics::Color::rgb(148, 0, 222), // Crateria
        1 => graphics::Color::rgb(0, 148, 0),  // Brinstar
        2 => graphics::Color::rgb(189, 0, 0),  // Norfair
        3 => graphics::Color::rgb(132, 140, 0), // Wrecked Ship
        4 => graphics::Color::rgb(25, 99, 239), // Maridia
        5 => graphics::Color::rgb(173, 99, 0), // Tourian
        _ => panic!("Unexpected area {}", area),
    };
    let hot_area_color = match area {
        0 => graphics::Color::rgb(222, 123, 255), // Crateria
        1 => graphics::Color::rgb(99, 206, 99), // Brinstar
        2 => graphics::Color::rgb(255, 99, 99), // Norfair
        3 => graphics::Color::rgb(189, 189, 90), // Wrecked Ship
        4 => graphics::Color::rgb(99, 165, 255), // Maridia
        5 => graphics::Color::rgb(239, 140, 99), // Tourian
        _ => panic!("Unexpected area {}", area),
    };
    match value {
        0 => graphics::Color::BLACK,
        1 => cool_area_color,
        2 => hot_area_color,
        3 => graphics::Color::WHITE,  // Wall/passage (white)
        4 => graphics::Color::BLACK, // Opaque black (used in elevators, covers up dotted grid background)
        6 => graphics::Color::rgb(239, 123, 0), // Yellow (orange) door (Power Bomb, Spazer)
        7 => graphics::Color::rgb(222, 16, 222), // Red (pink) door (Missile, Wave)
        8 => graphics::Color::rgb(33, 107, 255), // Blue door (Ice)
        12 => graphics::Color::BLACK, // Door lock shadow covering wall (black)
        13 => graphics::Color::WHITE, // Item dots (white)
        14 => graphics::Color::rgb(58, 255, 58), // Green door (Super, Plasma)
        15 => graphics::Color::rgb(148, 99, 115), // Gray door (including Charge)
        _ => panic!("Unexpected color value {}", value),
    }
}

fn load_textures(game_data: &GameData) -> Result<(ImplUserTexSource, Vec<usize>)> {
    let mut user_tex_source = ImplUserTexSource::new();

    let img_items = graphics::Image::from_file("../visualizer/items.png")?;
    let tex_items = graphics::Texture::from_image(&img_items, IntRect::default())?;
    let tex_item_width = (tex_items.size().x / 24) as i32;

    let img_doors = generate_door_sprites()?;
    let img_door_width = (img_doors.size().x / 10) as i32;

    let tex_helm = graphics::Texture::from_file("../visualizer/helm.png")?;
    let tex_bosses = graphics::Texture::from_file("../visualizer/bosses.png")?;
    let tex_minibosses = graphics::Texture::from_file("../visualizer/minibosses.png")?;
    let tex_misc = graphics::Texture::from_file("../visualizer/misc.png")?;

    user_tex_source.add_texture(Placeable::Helm as u64, tex_helm);
    user_tex_source.add_texture(UserTexId::FlagTypeBosses as u64, tex_bosses);
    user_tex_source.add_texture(UserTexId::FlagTypeMinibosses as u64, tex_minibosses);
    user_tex_source.add_texture(UserTexId::FlagTypeMisc as u64, tex_misc);

    // Add item textures to egui
    for i in 0..22 {
        let source_rect = IntRect::new(i * tex_item_width, 0, tex_item_width, img_items.size().y as i32);
        let tex = graphics::Texture::from_image(&img_items, source_rect)?;
        user_tex_source.add_texture(Placeable::ETank as u64 + i as u64, tex);
    }
    // Add Door textures to egui
    for i in 0..10 {
        let source_rect = IntRect::new(i * img_door_width, 0, img_door_width, img_doors.size().y as i32);
        let tex = graphics::Texture::from_image(&img_doors, source_rect)?;
        user_tex_source.add_texture(Placeable::DoorMissile as u64 + i as u64, tex);
    }
    // Add Flag textures to egui
    let mut flag_has_tex = Vec::new();
    for flag_idx in 0..game_data.flag_ids.len() {
        let flag_id = game_data.flag_ids[flag_idx];
        let flag_str = &game_data.flag_isv.keys[flag_id];
        if let Ok(tex) = graphics::Texture::from_file(("../visualizer/".to_string().to_owned() + flag_str + ".png").as_str()) {
            user_tex_source.add_texture(UserTexId::FlagFirst as u64 + flag_id as u64, tex);
            flag_has_tex.push(flag_id);
        }
    }

    Ok((user_tex_source, flag_has_tex))
}

// Creates a texture atlas and maps all rooms into it
fn load_room_sprites(game_data: &GameData) -> Result<(FBox<graphics::Image>, Vec<RoomData>)> {
    let mut image_mappings = Vec::new();

    for map_tile_data in &game_data.map_tile_data {
        let mut max_width = 0;
        let mut max_height = 0;

        for tile in &map_tile_data.map_tiles {
            max_width = max(max_width, tile.coords.0 + 1);
            max_height = max(max_height, tile.coords.1 + 1);
        }

        let mut image = graphics::Image::new_solid(8 * max_width as u32, 8 * max_height as u32, graphics::Color::TRANSPARENT)?;
        for tile in &map_tile_data.map_tiles {
            let x_offset = tile.coords.0 as u32 * 8;
            let y_offset = tile.coords.1 as u32 * 8;
            
            // Don't render liquid levels in elevators
            if !tile.special_type.is_some_and(|x| x == MapTileSpecialType::Elevator) {
                if let Some(liquid_level) = tile.liquid_level {
                    let start_index = (liquid_level * 8.0).round() as u32;
                    for y in start_index..8 {
                        for x in 0..8 {
                            if (x + y) % 2 == 0 {
                                image.set_pixel(x_offset + x, y_offset + y, graphics::Color::BLACK)?;
                            }
                        }
                    }
                }
            }

            if let Some(special_type) = tile.special_type {
                if special_type == MapTileSpecialType::Elevator || special_type == MapTileSpecialType::Tube {
                    for i in 0..8 {
                        image.set_pixel(x_offset, y_offset + i, graphics::Color::BLACK)?;
                        image.set_pixel(x_offset + 7, y_offset + i, graphics::Color::BLACK)?;
                        image.set_pixel(x_offset + 3, y_offset + i, graphics::Color::BLACK)?;
                        image.set_pixel(x_offset + 4, y_offset + i, graphics::Color::BLACK)?;

                        if i % 2 == 1 {
                            image.set_pixel(x_offset + 2, y_offset + i, graphics::Color::BLACK)?;
                            image.set_pixel(x_offset + 5, y_offset + i, graphics::Color::BLACK)?;
                        }

                        image.set_pixel(x_offset + 1, y_offset + i, graphics::Color::WHITE)?;
                        image.set_pixel(x_offset + 6, y_offset + i, graphics::Color::WHITE)?;
                    }
                } else if special_type == MapTileSpecialType::Black {
                    for y in 0..8 {
                        for x in 0..8 {
                            image.set_pixel(x_offset + x, y_offset + y, graphics::Color::BLACK)?;
                        }
                    }
                } else {
                    let comp = if special_type == MapTileSpecialType::SlopeDownCeilingHigh ||
                        special_type == MapTileSpecialType::SlopeDownCeilingLow ||
                        special_type == MapTileSpecialType::SlopeUpCeilingHigh ||
                        special_type == MapTileSpecialType::SlopeUpCeilingLow {
                        |l: i32, r: i32| l > r
                    } else {
                        |l: i32, r: i32| l < r
                    };
                    let x_fn = if special_type == MapTileSpecialType::SlopeUpFloorHigh ||
                    special_type == MapTileSpecialType::SlopeUpFloorLow ||
                    special_type == MapTileSpecialType::SlopeUpCeilingHigh ||
                    special_type == MapTileSpecialType::SlopeUpCeilingLow {
                        if special_type == MapTileSpecialType::SlopeUpCeilingLow ||
                            special_type == MapTileSpecialType::SlopeUpFloorLow ||
                            special_type == MapTileSpecialType::SlopeDownCeilingLow ||
                            special_type == MapTileSpecialType::SlopeDownFloorLow {
                            |x: i32| 7 - ((x as f32) * 0.5).floor() as i32
                        } else {
                            |x: i32| 3 - ((x as f32) * 0.5).floor() as i32
                        }
                    } else {
                        if special_type == MapTileSpecialType::SlopeUpCeilingLow ||
                            special_type == MapTileSpecialType::SlopeUpFloorLow ||
                            special_type == MapTileSpecialType::SlopeDownCeilingLow ||
                            special_type == MapTileSpecialType::SlopeDownFloorLow {
                            |x: i32| ((x as f32) * 0.5 + 4.0).floor() as i32
                        } else {
                            |x: i32| ((x as f32) * 0.5).floor() as i32
                        }
                    };

                    for y in 0..8 {
                        for x in 0..8 {
                            if comp(x_fn(x), y) {
                                image.set_pixel(x_offset + x as u32, y_offset + y as u32, graphics::Color::BLACK)?;
                            } else if x_fn(x) == y {
                                image.set_pixel(x_offset + x as u32, y_offset + y as u32, graphics::Color::WHITE)?;
                            }
                        }
                    }
                }
            }
        
            if tile.interior == MapTileInterior::ElevatorPlatformHigh || tile.interior == MapTileInterior::ElevatorPlatformLow {
                let y = if tile.interior == MapTileInterior::ElevatorPlatformLow { 5 } else { 2 };
                image.set_pixel(x_offset + 3, y_offset + y, graphics::Color::WHITE)?;
                image.set_pixel(x_offset + 4, y_offset + y, graphics::Color::WHITE)?;
            } else if tile.interior == MapTileInterior::Item || tile.interior == MapTileInterior::DoubleItem {
                
            } else {
                let room_type = match tile.interior {
                    MapTileInterior::EnergyRefill => Some(SpecialRoom::EnergyRefill),
                    MapTileInterior::AmmoRefill => Some(SpecialRoom::AmmoRefill),
                    MapTileInterior::DoubleRefill | MapTileInterior::Ship => Some(SpecialRoom::FullRefill),
                    MapTileInterior::SaveStation => Some(SpecialRoom::SaveStation),
                    MapTileInterior::MapStation => Some(SpecialRoom::MapStation),
                    _ => None
                };
                if room_type.is_some() {
                    let mask = get_special_room_mask(room_type.unwrap());
                    for y in 0..8 {
                        for x in 0..8 {
                            if mask[y][x] == 1 {
                                image.set_pixel(x_offset + x as u32, y_offset + y as u32, graphics::Color::WHITE)?;
                            }
                        }
                    }
                }
            }

            for i in 0..4 {
                let x1 = x_offset + i;
                let x2 = x_offset + 7 - i;
                let y1 = y_offset + i;
                let y2 = y_offset + 7 - i;

                if tile.left == MapTileEdge::Wall || tile.left == MapTileEdge::QolWall ||
                    ((tile.left == MapTileEdge::Door || tile.left == MapTileEdge::QolDoor) && i < 3) ||
                    (tile.left != MapTileEdge::Empty && tile.left != MapTileEdge::QolEmpty && i < 2) {
                    image.set_pixel(x_offset, y1, graphics::Color::WHITE)?;
                    image.set_pixel(x_offset, y2, graphics::Color::WHITE)?;
                }
                if tile.right == MapTileEdge::Wall || tile.right == MapTileEdge::QolWall ||
                    ((tile.right == MapTileEdge::Door || tile.right == MapTileEdge::QolDoor) && i < 3) ||
                    (tile.right != MapTileEdge::Empty && tile.right != MapTileEdge::QolEmpty && i < 2) {
                    image.set_pixel(x_offset + 7, y1, graphics::Color::WHITE)?;
                    image.set_pixel(x_offset + 7, y2, graphics::Color::WHITE)?;
                }
                if tile.top == MapTileEdge::Wall || tile.top == MapTileEdge::QolWall ||
                    ((tile.top == MapTileEdge::Door || tile.top == MapTileEdge::QolDoor) && i < 3) ||
                    (tile.top != MapTileEdge::Empty && tile.top != MapTileEdge::QolEmpty && i < 2) {
                    image.set_pixel(x1, y_offset, graphics::Color::WHITE)?;
                    image.set_pixel(x2, y_offset, graphics::Color::WHITE)?;
                }
                if tile.bottom == MapTileEdge::Wall || tile.bottom == MapTileEdge::QolWall ||
                    ((tile.bottom == MapTileEdge::Door || tile.bottom == MapTileEdge::QolDoor) && i < 3) ||
                    (tile.bottom != MapTileEdge::Empty && tile.bottom != MapTileEdge::QolEmpty && i < 2) {
                    image.set_pixel(x1, y_offset + 7, graphics::Color::WHITE)?;
                    image.set_pixel(x2, y_offset + 7, graphics::Color::WHITE)?;
                }
            }
        }

        let room_ptr = &game_data.room_ptr_by_id[&map_tile_data.room_id];
        let room_idx = game_data.room_idx_by_ptr[room_ptr];

        image_mappings.push((RoomData {
            room_id: map_tile_data.room_id,
            room_idx: room_idx,
            tile_width: max_width as u32,
            tile_height: max_height as u32,
            atlas_x_offset: 0,
            atlas_y_offset: 0
        }, image));
    }
    
    // Next we sort the image array by size
    image_mappings.sort_by(|l, r| {
        if l.1.size().y == r.1.size().y {
            return r.1.size().x.cmp(&l.1.size().x);
        }
        r.1.size().y.cmp(&l.1.size().y)
    });

    // Create an atlas prototype out of rectangles to apply a scanline algorithm to
    let mut atlas_prototype = Vec::<graphics::Rect<u32>>::new();
    let mut atlas_width = 0 as u32;
    let mut atlas_height = 0 as u32;

    // Guess an initial atlas width
    let mut atlas_width_guess = 32 as u32;

    // Continuously try to create an atlas until it is close to being square
    loop {
        for i in 0..image_mappings.len() {
            let room_data = &mut image_mappings[i].0;

            if atlas_prototype.is_empty() {
                atlas_prototype.push(graphics::Rect::<u32>::new(0, 0, room_data.tile_width, room_data.tile_height));
                atlas_width = max(atlas_width, room_data.tile_width);
                atlas_height = max(atlas_height, room_data.tile_height);
                room_data.atlas_x_offset = 0;
                room_data.atlas_y_offset = 0;
                continue;
            }

            let mut y = 0u32;
            // Scanline the atlas
            'outer: loop {
                for x in 0..=(atlas_width_guess - room_data.tile_width) {
                    let mut has_space = true;
                    let rect_to_push = graphics::Rect::<u32>::new(x, y, room_data.tile_width, room_data.tile_height);
                    for j in 0..atlas_prototype.len() {
                        // If the image would fit at this spot we put it here
                        let rect = atlas_prototype[j];
                        if rect.intersection(&rect_to_push).is_some() {
                            has_space = false;
                            break;
                        }
                    }
                    if has_space {
                        atlas_prototype.push(rect_to_push);
                        atlas_width = max(atlas_width, x + room_data.tile_width);
                        atlas_height = max(atlas_height, y + room_data.tile_height);
                        room_data.atlas_x_offset = x;
                        room_data.atlas_y_offset = y;
                        break 'outer;
                    }
                }
                y += 1;
            }
        }
        // If the atlas has skipped over being square we are done
        if atlas_height < atlas_width_guess {
            break;
        }
        // Otherwise guess a new width and repeat
        atlas_width_guess += 32;

        atlas_width = 0;
        atlas_height = 0;
        atlas_prototype.clear();
    }

    // Convert the atlas prototype into a real atlas
    let mut atlas = graphics::Image::new_solid(atlas_width * 8, atlas_height * 8, graphics::Color::TRANSPARENT)?;
    for i in 0..image_mappings.len() {
        let room_data = &image_mappings[i].0;
        let image = &image_mappings[i].1;
        atlas.copy_image(&image, room_data.atlas_x_offset * 8, room_data.atlas_y_offset * 8,
            graphics::Rect::default(),
            true);
    }

    let (mut room_data, _): (Vec<_>, Vec<_>) = image_mappings.into_iter().unzip();

    // Move toilet to the front so we always draw it below
    let toilet_idx = room_data.iter().position(|x| x.room_id == 321).unwrap();
    room_data.swap(0, toilet_idx);

    Ok((atlas, room_data))
}

// Generates 9 door sprites (Missile, Super, PB, Charge, Ice, Wave, Spazer, Plasma, Gray)
fn generate_door_sprites() -> Result<FBox<graphics::Image>> {
    let mut img_doors = graphics::Image::new_solid(3 * 10, 8, Color::TRANSPARENT).unwrap();
    for x in 0..10 {
        let door_color_index = match x {
            0 | 4 => 7, // Missile | Wave
            1 | 6 => 14, // Super | Plasma
            2 | 3 => 6, // PB | Spazer
            7 | 9 => 15, // Charge | Gray
            5 => 8, // Ice
            8 => 3, // Wall Doors
            _ => 15
        };
        
        img_doors.set_pixel(3 * x + 2, 3, get_explored_color(12, 0))?;
        img_doors.set_pixel(3 * x + 2, 4, get_explored_color(12, 0))?;
        
        img_doors.set_pixel(3 * x, 3, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x + 1, 3, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x, 4, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x + 1, 4, get_explored_color(door_color_index, 0))?;
        
        if x < 3 || x == 8 || x == 9 {
            img_doors.set_pixel(3 * x, 2, get_explored_color(12, 0))?;
            img_doors.set_pixel(3 * x + 1, 1, get_explored_color(12, 0))?;
            img_doors.set_pixel(3 * x + 2, 2, get_explored_color(12, 0))?;
            img_doors.set_pixel(3 * x + 2, 5, get_explored_color(12, 0))?;
            img_doors.set_pixel(3 * x + 1, 6, get_explored_color(12, 0))?;
            img_doors.set_pixel(3 * x, 5, get_explored_color(12, 0))?;
            img_doors.set_pixel(3 * x + 1, 2, get_explored_color(door_color_index, 0))?;
            img_doors.set_pixel(3 * x + 1, 5, get_explored_color(door_color_index, 0))?;
        } else {
            img_doors.set_pixel(3 * x + 1, 2, get_explored_color(13, 0))?;
            img_doors.set_pixel(3 * x + 1, 5, get_explored_color(3, 0))?;
        }
    }
    Ok(img_doors)
}

fn draw_thick_line_strip(rt: &mut dyn RenderTarget, states: &RenderStates, strip: &[Vertex], thickness: f32) {
    for i in 1..strip.len() {
        let prev = &strip[i - 1];
        let curr = &strip[i];

        let mut circle_end = graphics::CircleShape::new(thickness / 2.0, 30);
        circle_end.set_origin(thickness / 2.0);

        circle_end.set_position(prev.position);
        circle_end.set_fill_color(prev.color);
        rt.draw_with_renderstates(&circle_end, states);

        circle_end.set_position(curr.position);
        circle_end.set_fill_color(curr.color);
        rt.draw_with_renderstates(&circle_end, states);

        let mut diff = curr.position - prev.position;
        diff /= diff.length_sq().sqrt() * 2.0;
        let ortho = diff.perpendicular();

        let rect: [Vertex; 4] = [
            Vertex::with_pos_color(prev.position + ortho * thickness, prev.color),
            Vertex::with_pos_color(prev.position - ortho * thickness, prev.color),
            Vertex::with_pos_color(curr.position + ortho * thickness, curr.color),
            Vertex::with_pos_color(curr.position - ortho * thickness, curr.color)
        ];
        rt.draw_primitives(&rect, PrimitiveType::TRIANGLE_STRIP, states);
    }
}

enum FlagType {
    Bosses = Placeable::VARIANTS.len() as isize + 1,
    Minibosses,
    Misc
}

fn get_flag_info(flag: &String) -> Result<(f32, f32, FlagType, &str)> {
    match flag.as_str() {
        "f_MaridiaTubeBroken" => Ok((0.0, 1.0, FlagType::Misc, "Break Maridia Tube")),
        "f_ShaktoolDoneDigging" => Ok((1.5, 0.0, FlagType::Misc, "Clear Shaktool Room")),
        "f_UsedAcidChozoStatue" => Ok((0.0, 0.0, FlagType::Misc, "Use Acid Statue")),
        "f_UsedBowlingStatue" => Ok((4.0, 1.0, FlagType::Misc, "Use Bowling Statue")),
        "f_ClearedPitRoom" => Ok((1.0, 0.0, FlagType::Misc, "Clear Pit Room")),
        "f_ClearedBabyKraidRoom" => Ok((2.5, 0.0, FlagType::Misc, "Clear Baby Kraid Room")),
        "f_ClearedPlasmaRoom" => Ok((0.5, 1.0, FlagType::Misc, "Clear Plasma Room")),
        "f_ClearedMetalPiratesRoom" => Ok((1.0, 0.0, FlagType::Misc, "Clear Metal Pirates Room")),
        "f_DefeatedBombTorizo" => Ok((0.5, 0.0, FlagType::Minibosses, "Defeat Bomb Torizo")),
        "f_DefeatedBotwoon" => Ok((0.5, 0.0, FlagType::Minibosses, "Defeat Botwoon")),
        "f_DefeatedCrocomire" => Ok((4.0, 0.0, FlagType::Minibosses, "Defeat Crocomire")),
        "f_DefeatedSporeSpawn" => Ok((0.0, 1.5, FlagType::Minibosses, "Defeat Spore Spawn")),
        "f_DefeatedGoldenTorizo" => Ok((0.5, 1.0, FlagType::Minibosses, "Defeat Golden Torizo")),
        "f_DefeatedKraid" => Ok((0.5, 0.5, FlagType::Bosses, "Defeat Kraid")),
        "f_DefeatedPhantoon" => Ok((0.0, 0.0, FlagType::Bosses, "Defeat Phantoon")),
        "f_DefeatedDraygon" => Ok((0.5, 0.5, FlagType::Bosses, "Defeat Draygon")),
        "f_DefeatedRidley" => Ok((0.0, 0.5, FlagType::Bosses, "Defeat Ridley")),
        "f_KilledMetroidRoom1" => Ok((2.5, 0.0, FlagType::Misc, "Clear Metroid Room 1")),
        "f_KilledMetroidRoom2" => Ok((0.0, 0.5, FlagType::Misc, "Clear Metroid Room 2")),
        "f_KilledMetroidRoom3" => Ok((2.5, 0.0, FlagType::Misc, "Clear Metroid Room 3")),
        "f_KilledMetroidRoom4" => Ok((0.0, 0.5, FlagType::Misc, "Clear Metroid Room 4")),
        "f_DefeatedMotherBrain" => Ok((1.5, 0.0, FlagType::Bosses, "Defeat Mother Brain")),
        _ => bail!("Invalid flag \"{}\"", flag)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
enum SpoilerType {
    None,
    Hub,
    Item(usize),
    Flag(usize)
}

#[repr(u64)]
enum UserTexId {
    DoorGray = Placeable::VARIANTS.len() as u64,
    FlagTypeBosses,
    FlagTypeMinibosses,
    FlagTypeMisc,
    FlagFirst,
}

#[derive(PartialEq, Eq, Clone)]
enum ModalType {
    None,
    Error(String),
    Status(String),
    Info(String)
}

struct View {
    x_offset: f32,
    y_offset: f32,
    zoom: f32,
    window_size: Vector2f
}

impl View {
    fn new() -> View {
        View {
            x_offset: 0.0,
            y_offset: 0.0,
            zoom: 1.0,
            window_size: Vector2f::default()
        }
    }

    fn to_local_coords(&self, x: f32, y: f32) -> (f32, f32) {
        let new_x = (x - self.x_offset) / self.zoom;
        let new_y = (y - self.y_offset) / self.zoom;
        (new_x, new_y)
    }

    fn focus_rect(&mut self, rect: FloatRect) {
        let center_rect = rect.position() + rect.size() / 2.0;
        let center_screen = self.window_size / 2.0;
        self.x_offset = center_screen.x - center_rect.x * self.zoom;
        self.y_offset = center_screen.y - center_rect.y * self.zoom;
    }
}

#[derive(PartialEq, Eq, Clone, Copy, VariantArray)]
enum Hotkeys {
    IncrementStep,
    DecrementStep,
    UpdateSpoiler,
    OpenSpoilerOverride,
    ToggleAutoSpoiler,
    EraseSelection,
    //Undo,
    //Redo
}

impl Hotkeys {
    fn to_keybind(&self) -> Keybind {
        let id = *self as usize;
        match *self {
            Hotkeys::IncrementStep => Keybind::new(id, "Increment Spoiler Step", "Increments the current spoiler step", vec![Key::Add]),
            Hotkeys::DecrementStep => Keybind::new(id, "Decrement Spoiler Step", "Decrement the current spoiler step", vec![Key::Subtract]),
            Hotkeys::UpdateSpoiler => Keybind::new(id, "Update Spoiler Log", "Manually updates the spoiler log", vec![Key::F5]),
            Hotkeys::OpenSpoilerOverride => Keybind::new(id, "Open Spoiler Overrides", "Opens the Spoiler Overrides window for the current step", vec![Key::F6]),
            Hotkeys::ToggleAutoSpoiler => Keybind::new(id, "Toggle Auto-Spoiler", "Toggles the automatic spoiler update setting", vec![Key::F7]),
            Hotkeys::EraseSelection => Keybind::new(id, "Erase selected rooms", "Erases the selected rooms from the map", vec![Key::Delete]),
            //Hotkeys::Undo => Keybind::new(id, "Undo", "Undoes the last action", vec![Key::LControl, Key::Z]),
            //Hotkeys::Redo => Keybind::new(id, "Redo", "Redoes the last action", vec![Key::LControl, Key::Y]),
        }
    }
}

struct PlandoApp {
    plando: Plando,
    settings: Settings,
    settings_path: String,
    rom_vanilla: Option<Rom>,
    user_tex_source: ImplUserTexSource,
    flag_has_tex: Vec<usize>,
    obj_room_map: HashMap<usize, Objective>,
    room_data: Vec<RoomData>,
    atlas_tex: FBox<graphics::Texture>,

    tex_base_map: FBox<graphics::RenderTexture>,

    view: View,

    key_state: KeyState,
    mouse_state: MouseState,
    local_mouse_x: f32,
    local_mouse_y: f32,
    is_mouse_public: bool,
    click_consumed: bool,

    spoiler_step: usize,
    spoiler_type: SpoilerType,
    modal_type: ModalType,
    override_window: Option<usize>,

    layout: Layout,
    logic_customization: LogicCustomization,
    settings_customization: SettingsCustomize,
    map_editor: MapEditorUi,
    room_search: RoomSearch,

    global_timer: u64
}

impl PlandoApp {
    //const GRID_SIZE: usize = 72;

    fn new() -> Result<PlandoApp> {
        let game_data = GameData::load()?;
        let preset_data = load_preset_data(&game_data)?;

        let settings_path = Path::new("../plando_settings.json");
        let settings = match load_settings(settings_path, &preset_data) {
            Ok(settings) => settings,
            Err(err) => {
                println!("{}", err.to_string());
                Settings::default()
            }
        };
        if settings.auto_update {
            match check_update() {
                Ok(_) => {
                    bail!("Please restart application for changes to take effect");
                },
                Err(err) => println!("{}", err.to_string())
            };
        }

        let mut plando = Plando::new(game_data, preset_data.default_preset.clone(), &preset_data)?;

        let preset = match &settings.last_logic_preset {
            Some(preset) => preset.clone(),
            None => preset_data.default_preset.clone()
        };

        plando.load_preset(preset.clone());

        let logic_customization = LogicCustomization::new(preset_data, preset);

        let mut settings_customization = SettingsCustomize::new()?;
        settings_customization.customization = settings.last_customization.clone();

        let rom_vanilla_path = Path::new(&settings.rom_path);
        let rom_vanilla = load_vanilla_rom(rom_vanilla_path).ok();

        let (user_tex_source, flag_has_tex) = load_textures(&plando.game_data)?;

        let obj_room_map: HashMap<usize, Objective> = vec![
            Objective::Kraid,
            Objective::Phantoon,
            Objective::Draygon,
            Objective::Ridley,
            Objective::SporeSpawn,
            Objective::Crocomire,
            Objective::Botwoon,
            Objective::GoldenTorizo,
            Objective::MetroidRoom1,
            Objective::MetroidRoom2,
            Objective::MetroidRoom3,
            Objective::MetroidRoom4,
            Objective::BombTorizo,
            Objective::BowlingStatue,
            Objective::AcidChozoStatue,
            Objective::PitRoom,
            Objective::BabyKraidRoom,
            Objective::PlasmaRoom,
            Objective::MetalPiratesRoom,
        ].into_iter().map(
            |obj| {
                let flag_id = plando.game_data.flag_isv.index_by_key[obj.get_flag_name()];
                let flag_idx = plando.game_data.flag_ids.iter().position(|x| *x == flag_id).unwrap();
                let vertex = plando.game_data.flag_vertex_ids[flag_idx][0];
                let vertex_info = plando.get_vertex_info(vertex);
                (vertex_info.room_id, obj)
            }
        ).collect();

        let (atlas_img, room_data) = load_room_sprites(&plando.game_data)?;
        let atlas_tex = graphics::Texture::from_image(&atlas_img, IntRect::default())?;

        let mut mouse_state = MouseState::default();
        mouse_state.click_pos_leniency = settings.mouse_click_pos_tolerance as f32;
        mouse_state.click_time_leniency = settings.mouse_click_delay_tolerance as u32;

        let mut layout = Layout::new()?;
        for hotkey in Hotkeys::VARIANTS {
            let mut keybind = hotkey.to_keybind();
            if let Some(bind) = settings.hotkeys.get(&(*hotkey as usize)) {
                keybind.bind = HashSet::from_iter(bind.iter().cloned());
            }
            
            layout.hotkey_settings.add_keybind(keybind);
        }

        Ok(PlandoApp {
            plando,
            settings,
            settings_path: settings_path.as_os_str().to_str().unwrap().to_string(),
            rom_vanilla,
            user_tex_source,
            flag_has_tex,
            obj_room_map,
            room_data,
            atlas_tex,

            tex_base_map: graphics::RenderTexture::new(MapEditor::MAP_MAX_SIZE as u32 * 8, MapEditor::MAP_MAX_SIZE as u32 * 8)?,

            view: View::new(),

            key_state: KeyState::new(),
            mouse_state,
            local_mouse_x: 0.0,
            local_mouse_y: 0.0,
            is_mouse_public: true,
            click_consumed: false,

            spoiler_step: 0,
            spoiler_type: SpoilerType::None,
            modal_type: ModalType::None,
            override_window: None,

            layout,
            logic_customization,
            settings_customization,
            map_editor: MapEditorUi::default(),
            room_search: RoomSearch::default(),

            global_timer: 0
        })
    }

    fn update_settings(&mut self) {
        self.settings.hotkeys = self.layout.hotkey_settings.get_hotkeys().iter().map(|bind| (bind.id, bind.bind.iter().cloned().collect())).collect();
    }

    fn global_seconds(&self) -> f32 {
        self.global_timer as f32 / self.settings.fps_cap as f32
    }

    fn patch_rom(&mut self, save_path: &Path) -> Result<()> {
        let rom_vanilla = self.rom_vanilla.as_ref().ok_or(anyhow!("Provided no vanilla ROM to patch"))?;
        let settings = self.settings_customization.get_settings();

        let new_rom = self.plando.patch_rom(rom_vanilla, settings, &self.settings_customization.samus_sprite_categories, &self.settings_customization.mosaic_themes)?;

        let mut file = File::create(save_path)?;
        file.write_all(&new_rom.data)?;

        Ok(())
    }

    fn redraw_map(&mut self) {
        let draw_subareas = self.layout.sidebar_tab == SidebarPanel::Areas;

        let mut img_obj = graphics::Image::new_solid(8, 8, Color::TRANSPARENT).unwrap();
        let img_obj_mask = get_special_room_mask(SpecialRoom::Objective);
        for y in 0..8 {
            for x in 0..8 {
                if img_obj_mask[x][y] == 1 {
                    img_obj.set_pixel(x as u32, y as u32, Color::WHITE).unwrap();
                }
            }
        }
        let tex_obj = graphics::Texture::from_image(&img_obj, IntRect::default()).unwrap();

        let mut max_x = 0;
        let mut max_y = 0;
        for (idx, &(room_x, room_y)) in self.plando.map().rooms.iter().enumerate() {
            let room_geometry = &self.plando.game_data.room_geometry[idx];
            let room_width = room_geometry.map[0].len();
            let room_height = room_geometry.map.len();

            let room_right = room_x + room_width;
            let room_bottom = room_y + room_height;

            max_x = max_x.max(room_right);
            max_y = max_y.max(room_bottom);
        }
        self.tex_base_map.recreate(max_x as u32 * 8, max_y as u32 * 8, &ContextSettings::default()).unwrap();

        self.tex_base_map.clear(Color::TRANSPARENT);

        for i in 0..self.room_data.len() {
            let data = &self.room_data[i];

            if !self.plando.map().room_mask[data.room_idx] {
                continue;
            }

            let (x, y) = self.plando.map().rooms[data.room_idx];
            let room_geometry = &self.plando.game_data.room_geometry[data.room_idx];

            let is_objective = data.room_id == 238 || self.obj_room_map.get(&data.room_id).is_some_and(
                |obj| self.plando.objectives.contains(obj)
            );

            // Draw the background color
            for (local_y, row) in room_geometry.map.iter().enumerate() {
                for (local_x, &cell) in row.iter().enumerate() {
                    // Ignore map tiles on toilet
                    if cell == 0 && data.room_id != 321 {
                        continue;
                    }

                    let mut color_div = 1;
                    if !self.settings.disable_logic && !draw_subareas {
                        if let Some((_r, spoiler_log)) = &self.plando.randomization {
                            if spoiler_log.all_rooms[data.room_idx].map_bireachable_step[local_y][local_x] > self.spoiler_step as u8 {
                                color_div *= 2;
                            }
                            if spoiler_log.all_rooms[data.room_idx].map_reachable_step[local_y][local_x] > self.spoiler_step as u8 {
                                color_div *= 3;
                            }
                        }
                    }

                    let mut cell_color = if draw_subareas {
                        self.plando.map_editor.get_area_value(data.room_idx).to_color()
                    } else {
                        let color_value = if room_geometry.heated { 2 } else { 1 };
                        get_explored_color(color_value, self.plando.map().area[data.room_idx])
                    };
                    cell_color.r /= color_div;
                    cell_color.g /= color_div;
                    cell_color.b /= color_div;
                
                    let cell_x = (local_x + x) * 8;
                    let cell_y = (local_y + y) * 8;

                    let mut bg_rect = graphics::RectangleShape::with_size(Vector2f::new(8.0, 8.0));
                    bg_rect.set_position(Vector2f::new(cell_x as f32, cell_y as f32));
                    bg_rect.set_fill_color(cell_color);
                    self.tex_base_map.draw(&bg_rect);

                    // Draw Tile Outline
                    let sprite_tile_rect = IntRect::new(8 * (data.atlas_x_offset as i32 + local_x as i32), 8 * (data.atlas_y_offset as i32 + local_y as i32), 8, 8);
                    let mut sprite_tile = graphics::Sprite::with_texture_and_rect(&self.atlas_tex, sprite_tile_rect);
                    sprite_tile.set_position(Vector2f::new(cell_x as f32, cell_y as f32));
                    sprite_tile.set_color(Color::rgb(255 / color_div, 255 / color_div, 255 / color_div));
                    self.tex_base_map.draw(&sprite_tile);

                    if is_objective && get_objective_mask(data.room_id, local_x, local_y) {
                        sprite_tile.set_texture(&tex_obj, true);
                        self.tex_base_map.draw(&sprite_tile);
                    }

                    let (tex_helm_w, _, tex_helm) = self.user_tex_source.get_texture(Placeable::Helm as u64);
                    let mut sprite_helm = graphics::Sprite::with_texture(tex_helm);
                    sprite_helm.set_scale(8.0 / tex_helm_w);
                }
            }
        }

        self.tex_base_map.display();
    }

    fn render_loop(&mut self) {

        let version_number = "v".to_string() + cargo_crate_version!();

        let mut window = RenderWindow::new((1080, 720), &format!("Maprando Plando {version_number}"), Style::DEFAULT, &Default::default()).expect("Could not create Window");
        window.set_vertical_sync_enabled(false);
        window.set_key_repeat_enabled(false);

        let font_default = {
            let font = FontDefinitions::default();
            let font_data = &font.font_data["Hack"];
            match &font_data.font {
                std::borrow::Cow::Borrowed(font) => graphics::Font::from_memory_static(font),
                std::borrow::Cow::Owned(_) => panic!("Could not load default Font"),
            }
        }.unwrap();

        let tex_items = graphics::Texture::from_file("../visualizer/items.png").unwrap();

        let mut tex_grid = graphics::Texture::from_file("../visualizer/grid.png").unwrap();
        tex_grid.set_repeated(true);

        let mut cur_settings = self.plando.randomizer_settings.clone();

        let mut sidebar_width = 0.0;

        let mut sfegui = SfEgui::new(&window);
        sfegui.get_context().all_styles_mut(|style| {
            let mut def = default_text_styles();
            for (_, font_id) in &mut def {
                font_id.size *= self.settings.ui_scale;
            }
            style.text_styles = def;
        });

        let mut settings_open = false;
        let mut customize_open = false;
        let mut customize_logic_open = false;
        let mut reset_after_patch = false;

        let mut sidebar_selection: Option<Placeable> = None;
        let mut spoiler_window_bounds = FloatRect::default();
        let mut spoiler_details_hovered = false;

        let mut download_thread_active = false;
        let mut download_thread_handle: JoinHandle<Result<(), anyhow::Error>> = thread::spawn(|| { Ok(()) });

        let mut frame_counter = 0;
        let mut start = Instant::now();
        let mut fps_text = graphics::Text::new("FPS: 0", &font_default, 12);

        let mut last_spoiler_step = self.spoiler_step;
        self.redraw_map();

        while window.is_open() {
            self.key_state.next_frame();
            self.mouse_state.next_frame();
            self.global_timer += 1;
            (self.local_mouse_x, self.local_mouse_y) = self.view.to_local_coords(self.mouse_state.mouse_x, self.mouse_state.mouse_y);
            self.click_consumed = false;
            self.view.window_size = window.size().as_other() - Vector2f::new(sidebar_width, 0.0);

            if download_thread_active {
                if download_thread_handle.is_finished() {
                    let res = download_thread_handle.join().unwrap();
                    match res {
                        Ok(_) => self.modal_type = ModalType::None,
                        Err(err) => self.modal_type = ModalType::Error(err.to_string())
                    }
                    download_thread_active = false;
                    download_thread_handle = thread::spawn(|| { Ok(()) }); // Reset the handle
                }
            }

            if self.spoiler_step != last_spoiler_step {
                self.redraw_map();
                last_spoiler_step = self.spoiler_step;
            }

            // mouse_is_public is true if the mouse is on the map view and not on some GUI
            self.is_mouse_public = !(spoiler_details_hovered || settings_open || customize_open || customize_logic_open || self.modal_type != ModalType::None
                || spoiler_window_bounds.contains(self.mouse_state.get_mouse_pos()) || self.mouse_state.mouse_x >= window.size().x as f32 - sidebar_width
                || !self.map_editor.dragged_room_idx.is_empty() || self.override_window.is_some() || self.layout.is_open_any());

            while let Some(ev) = window.poll_event() {
                sfegui.scroll_factor = self.settings.scroll_speed;
                sfegui.add_event(&ev);
                self.key_state.add_event(ev);
                self.mouse_state.add_event(ev);

                match ev {
                    Event::Closed => {
                        let settings_path_str = self.settings_path.clone();
                        let settings_path = Path::new(&settings_path_str);
                        self.update_settings();
                        let _ = save_settings(&self.settings, settings_path);
                        window.close();
                    },
                    Event::MouseWheelScrolled { wheel: _, delta, x, y } => {
                        if self.is_mouse_public {
                            let factor = 1.1;
                            if delta > 0.0 && self.view.zoom < 20.0 {
                                self.view.zoom *= factor;
                                self.view.x_offset -= (factor - 1.0) * (x as f32 - self.view.x_offset);
                                self.view.y_offset -= (factor - 1.0) * (y as f32 - self.view.y_offset);
                            } else if delta < 0.0 && self.view.zoom > 0.1 {
                                self.view.zoom /= factor;
                                self.view.x_offset += (1.0 - 1.0 / factor) * (x as f32 - self.view.x_offset);
                                self.view.y_offset += (1.0 - 1.0 / factor) * (y as f32 - self.view.y_offset);
                            }
                        }
                    },
                    Event::Resized { width, height } => {
                        window.set_view(&graphics::View::from_rect(graphics::Rect::new(0.0, 0.0, width as f32, height as f32)).unwrap());
                        self.view.window_size = window.size().as_other();
                    },
                    _ => {}
                }
            }

            let window_size = window.size();

            if !self.layout.is_open(WindowType::HotkeySettings) {
                for (idx, hotkey) in self.layout.hotkey_settings.get_hotkeys().iter().enumerate() {
                    if hotkey.is_pressed(&self.key_state) {
                        match Hotkeys::VARIANTS[idx] {
                            Hotkeys::IncrementStep => self.spoiler_step += 1,
                            Hotkeys::DecrementStep => if self.spoiler_step > 0 { self.spoiler_step -= 1 },
                            Hotkeys::UpdateSpoiler => {
                                if let Err(err) = self.plando.update_spoiler_data() {
                                    self.modal_type = ModalType::Error(err.to_string());
                                }
                            },
                            Hotkeys::OpenSpoilerOverride => self.override_window = Some(self.spoiler_step + 1),
                            Hotkeys::ToggleAutoSpoiler => {
                                self.settings.spoiler_auto_update ^= true;
                            },
                            Hotkeys::EraseSelection => {
                                self.map_editor.selected_room_idx.iter().for_each(|&idx| self.plando.erase_room(idx));
                                self.map_editor.selected_room_idx.clear();
                                self.redraw_map();
                            }
                            //Hotkeys::Undo => println!("Undo"),
                            //Hotkeys::Redo => println!("Redo"),
                        }
                        break;
                    }
                }
            }

            // Drag to pan the view
            if self.mouse_state.is_button_down(mouse::Button::Middle) && self.is_mouse_public {
                self.view.x_offset += self.mouse_state.mouse_dx;
                self.view.y_offset += self.mouse_state.mouse_dy;
            }

            spoiler_details_hovered = false;

            window.set_framerate_limit(self.settings.fps_cap);
            window.clear(Color::rgb(0x1F, 0x1F, 0x1F));

            let mut states = graphics::RenderStates::default();
            states.transform.translate(self.view.x_offset, self.view.y_offset);
            states.transform.scale(self.view.zoom, self.view.zoom);

            // Don't render map if we're patching from a seed file to not spoiler the user
            if !reset_after_patch {
                // Draw background grid
                if !self.settings.disable_bg_grid {
                    //let spr_bg_grid = graphics::Sprite::with_texture_and_rect(&tex_grid, IntRect::new(0, 0, 8 * PlandoApp::GRID_SIZE as i32, 8 * PlandoApp::GRID_SIZE as i32));
                    //window.draw_with_renderstates(&spr_bg_grid, &states);
                    self.draw_background_grid(&mut *window, &states, &tex_grid);
                }

                // Draw the entire map
                {
                    let spr_map = graphics::Sprite::with_texture(self.tex_base_map.texture());
                    window.draw_with_renderstates(&spr_map, &states);
                }

                // Draw Possible Start Locations
                self.draw_start_locations(&mut *window, &states, sidebar_selection);

                // Draw Doors
                self.draw_placed_doors(&mut *window, &states);

                // Draw Gray Doors
                self.draw_gray_doors(&mut *window, &states);

                // Draw items
                self.draw_items(&mut *window, &states, sidebar_selection, &tex_items);

                // Draw flags
                self.draw_flags(&mut *window, &states, sidebar_selection);

                // Handle map io after all other io has been handled, also draws errors and editor overlays
                if sidebar_selection.is_none() {
                    self.handle_map_io(&mut *window, &states);
                }
        
                // Draw Door hover
                self.draw_door_hover(&mut *window, &states, sidebar_selection);

                // Draw the info overlay
                let window_size = window.size();
                self.layout.info_overlay_builder.render(
                    &mut *window,
                    self.mouse_state.mouse_x,
                    self.mouse_state.mouse_y,
                    window_size.x as f32 - self.mouse_state.mouse_x - sidebar_width,
                    &font_default,
                    14
                );

                // Draw spoiler route
                self.draw_spoiler_route(&mut *window, &states);
            }

            // Reset spoiler step and type if click resulted in nothing
            if !self.click_consumed && self.mouse_state.button_clicked.is_some() && self.is_mouse_public {
                sidebar_selection = None;
                self.spoiler_type = SpoilerType::None;
            }
            if sidebar_selection.is_some() {
                self.spoiler_type = SpoilerType::None;
            }

            // Draw Menu Bar
            let gui = sfegui.run(&mut window, |_rt, ctx| {
                egui::TopBottomPanel::top("menu_file_main").show(ctx, |ui| {
                    egui::menu::bar(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("Save Seed").clicked() {
                                let file_opt = FileDialog::new()
                                    .set_title("Save Seed as JSON file")
                                    .set_directory("/")
                                    .add_filter("JSON File", &["json"])
                                    .save_file();
                                if let Some(file) = file_opt {
                                    let res = save_seed(&mut self.plando, file.as_path());
                                    if res.is_err() {
                                        self.modal_type = ModalType::Error(res.unwrap_err().to_string());
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.button("Load Seed").clicked() {
                                let file_opt = FileDialog::new()
                                    .set_title("Load seed from JSON file")
                                    .set_directory("/")
                                    .add_filter("JSON File", &["json"])
                                    .pick_file();
                                if let Some(file) = file_opt {
                                    match load_seed(&mut self.plando, file.as_path(), &self.logic_customization.preset_data) {
                                        Ok(_) => cur_settings = self.plando.randomizer_settings.clone(),
                                        Err(err) => self.modal_type = ModalType::Error(err.to_string())
                                    }
                                    self.redraw_map();
                                }
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Patch ROM").clicked() {
                                customize_open = true;
                                ui.close_menu();
                            }
                            if ui.button("Patch ROM from seed file").clicked() {
                                if let Some(file) = FileDialog::new()
                                .set_title("Select seed JSON file to load and patch")
                                .set_directory("/").add_filter("JSON File", &["json"]).pick_file() {
                                    match load_seed(&mut self.plando, &file, &self.logic_customization.preset_data) {
                                        Ok(_) => {
                                            customize_open = true;
                                            reset_after_patch = true;
                                        }
                                        Err(err) => self.modal_type = ModalType::Error(err.to_string())
                                    }
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.menu_button("Map", |ui| {
                            if ui.button("Reroll Map (Vanilla)").clicked() {
                                self.plando.reroll_map(MapRepositoryType::Vanilla).unwrap();
                                self.redraw_map();
                                ui.close_menu();
                            }
                            if ui.add_enabled(self.plando.does_repo_exist(MapRepositoryType::Standard), egui::Button::new("Reroll Map (Standard)")).clicked() {
                                self.plando.reroll_map(MapRepositoryType::Standard).unwrap();
                                self.redraw_map();
                            }
                            if ui.add_enabled(self.plando.does_repo_exist(MapRepositoryType::Wild), egui::Button::new("Reroll Map (Wild)")).clicked() {
                                self.plando.reroll_map(MapRepositoryType::Wild).unwrap();
                                self.redraw_map();
                            }
                            ui.separator();
                            if ui.button("Save Map to file").clicked() {
                                let file_opt = FileDialog::new()
                                    .set_title("Save Map to JSON file")
                                    .set_directory("/")
                                    .add_filter("JSON File", &["json"])
                                    .save_file();
                                if let Some(file) = file_opt {
                                    let res = self.plando.map_editor.save_map(file.as_path());
                                    if res.is_err() {
                                        self.modal_type = ModalType::Error(res.unwrap_err().to_string());
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.button("Load Map from file").clicked() {
                                let file_opt = FileDialog::new()
                                    .set_title("Select Map JSON to load")
                                    .set_directory("/")
                                    .add_filter("JSON File", &["json"])
                                    .pick_file();
                                if let Some(file) = file_opt {
                                    let res = self.plando.load_map_from_file(&file);
                                    if let Err(err) = res {
                                        self.modal_type = ModalType::Error(err.to_string())
                                    };
                                    self.redraw_map();
                                }
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.add_enabled(self.modal_type == ModalType::None, egui::Button::new("Download Map Repositories")).clicked() {
                                download_thread_handle = thread::spawn(download_map_repos);
                                download_thread_active = true;
                                self.modal_type = ModalType::Status("Downloading/Unpacking... This might take a while".to_string());
                            }
                        });
                        ui.menu_button("Items", |ui| {
                            if ui.button("Clear all Items").clicked() {
                                self.plando.clear_item_locations();
                                if self.settings.spoiler_auto_update {
                                    if let Err(err) = self.plando.update_spoiler_data() {
                                        self.modal_type = ModalType::Error(err.to_string());
                                    }
                                }
                            }
                            if ui.button("Clear all Doors").clicked() {
                                self.plando.clear_doors();
                                if self.settings.spoiler_auto_update {
                                    if let Err(err) = self.plando.update_spoiler_data() {
                                        self.modal_type = ModalType::Error(err.to_string());
                                    }
                                }
                            }
                            ui.separator();
                            if ui.button("Replace Nothings with Missiles").clicked() {
                                for i in 0..self.plando.item_locations.len() {
                                    if self.plando.item_locations[i] == Item::Nothing {
                                        let _ = self.plando.place_item(i, Item::Missile);
                                    }
                                }
                                if self.settings.spoiler_auto_update {
                                    if let Err(err) = self.plando.update_spoiler_data() {
                                        self.modal_type = ModalType::Error(err.to_string());
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui.button("Randomize Doors").clicked() {
                                self.plando.clear_doors();

                                let seed = (self.plando.rng.next_u64() & 0xFFFFFFFF) as usize;
                                let locked_door_data = maprando::randomize::randomize_doors(&self.plando.game_data, self.plando.map(), &self.plando.randomizer_settings, &self.plando.objectives, seed);
                                for door in locked_door_data.locked_doors {
                                    let (room_idx, door_idx) = self.plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door.src_ptr_pair];
                                    let _ = self.plando.place_door(room_idx, door_idx, Some(door.door_type), false, true);
                                }

                                if let Err(_) = self.plando.update_hub_location() {
                                    let _ = self.plando.place_start_location(Plando::get_ship_start());
                                    self.modal_type = ModalType::Error("The logical hub was blocked. Start location is defaulted to ship".to_string());
                                }

                                if self.settings.spoiler_auto_update {
                                    if let Err(err) = self.plando.update_spoiler_data() {
                                        self.modal_type = ModalType::Error(err.to_string());
                                    }
                                }
                            }

                            ui.separator();
                            if ui.button("Reset all Spoiler Overrides").clicked() {
                                self.plando.spoiler_overrides.clear();
                                if self.settings.spoiler_auto_update {
                                    let _ = self.plando.update_spoiler_data();
                                }
                            }
                        });
                        ui.menu_button("Settings", |ui| {
                            if ui.button("Plando Settings").clicked() {
                                settings_open = true;
                                ui.close_menu();
                            }
                            if ui.button("Logic Settings").clicked() {
                                customize_logic_open = true;
                                ui.close_menu();
                            }
                            if ui.button("Hotkeys").clicked() {
                                self.layout.open(WindowType::HotkeySettings);
                            }
                        });
                        if ui.button("Github").clicked() {
                            if let Err(err) = open::that("https://github.com/Noktuska/maprando-plando/blob/main/README.md") {
                                self.modal_type = ModalType::Error(err.to_string());
                            }
                        }
                    });
                });

                self.layout.render(ctx);

                // Draw item selection sidebar
                let max_sidebar_width = (window_size.x / 2) as f32;
                sidebar_width = egui::SidePanel::right("panel_item_select").width_range(128.0..=max_sidebar_width).show(ctx, |ui| {
                    self.draw_sidebar(ui, &mut sidebar_selection);
                }).response.rect.width();

                // Draw Spoiler Details Window
                if !reset_after_patch {
                    if self.spoiler_type != SpoilerType::None && self.plando.randomization.is_some() {
                        spoiler_details_hovered = self.draw_spoiler_details(ctx);
                    } else if self.plando.randomization.is_some() {
                        spoiler_window_bounds = self.draw_spoiler_summary(ctx, self.mouse_state.mouse_y as f32, spoiler_window_bounds);
                    }
                }

                if self.override_window.is_some() {
                    spoiler_details_hovered |= self.draw_spoiler_override(ctx);
                }

                if settings_open {
                    settings_open = self.draw_settings_window(ctx);
                }

                if customize_open {
                    match self.settings_customization.draw_customization_window(ctx) {
                        SettingsCustomizeResult::Idle => {},
                        SettingsCustomizeResult::Cancel => customize_open = false,
                        SettingsCustomizeResult::Apply => {
                            self.settings.last_customization = self.settings_customization.customization.clone();
                            if self.rom_vanilla.is_none() {
                                if let Some(file) = FileDialog::new().set_title("Select vanilla ROM")
                                .set_directory("/").add_filter("Snes ROM", &["sfc", "smc"]).pick_file() {
                                    self.settings.rom_path = file.to_str().unwrap().to_string();
                                    match load_vanilla_rom(&Path::new(&self.settings.rom_path)) {
                                        Ok(rom) => self.rom_vanilla = Some(rom),
                                        Err(err) => self.modal_type = ModalType::Error(err.to_string())
                                    }
                                }
                            }
                            if self.rom_vanilla.is_some() {
                                if let Some(file_out) = FileDialog::new().set_title("Select output location")
                                .set_directory("/")
                                .add_filter("Snes ROM", &["sfc"])
                                .save_file() {
                                    if let Err(err) = self.patch_rom(&file_out) {
                                        self.modal_type = ModalType::Error(err.to_string());
                                    }
                                    customize_open = false;
                                }
                            }
                        },
                        SettingsCustomizeResult::Error(err) => self.modal_type = ModalType::Error(err),
                    }

                    if !customize_open && reset_after_patch {
                        let _ = self.plando.reroll_map(MapRepositoryType::Vanilla);
                        let preset = match self.settings.last_logic_preset.as_ref() {
                            Some(preset) => preset.clone(),
                            None => self.logic_customization.preset_data.default_preset.clone()
                        };
                        self.plando.load_preset(preset);
                        reset_after_patch = false;
                    }
                }

                if customize_logic_open {
                    match self.logic_customization.draw_window(ctx) {
                        Ok(should_close) => if should_close {
                            customize_logic_open = false;
                            self.plando.load_preset(self.logic_customization.settings.clone());
                            self.settings.last_logic_preset = Some(self.logic_customization.settings.clone());
                            self.redraw_map();
                        }
                        Err(err) => self.modal_type = ModalType::Error(err.to_string())
                    }
                }

                match self.modal_type.clone() {
                    ModalType::None => {}
                    ModalType::Error(msg) => {
                        let modal = egui::Modal::new(Id::new("modal_error")).show(ctx, |ui| {
                            ui.set_min_width(256.0);
                            ui.heading("Error");
                            ui.label(msg);
                            if ui.button("OK").clicked() {
                                self.modal_type = ModalType::None;
                            }
                        });
                        if modal.should_close() {
                            self.modal_type = ModalType::None;
                        }
                    }
                    ModalType::Status(msg) => {
                        egui::Modal::new(Id::new("modal_status")).show(ctx, |ui| {
                            ui.set_min_width(256.0);
                            ui.heading("Status");
                            ui.label(msg);
                        });
                    }
                    ModalType::Info(msg) => {
                        let modal = egui::Modal::new(Id::new("modal_info")).show(ctx, |ui| {
                            ui.set_min_width(256.0);
                            ui.heading("Info");
                            ui.label(msg);
                            if ui.button("OK").clicked() {
                                self.modal_type = ModalType::None;
                            }
                        });
                        if modal.should_close() {
                            self.modal_type = ModalType::None;
                        }
                    }
                }
            }).unwrap();
            sfegui.draw(gui, &mut window, Some(&mut self.user_tex_source));

            // Draw current version number
            let mut version_text = graphics::Text::new(&version_number, &font_default, 12);
            version_text.set_fill_color(Color::rgba(0xAF, 0xAF, 0xAF, 0xCF));
            version_text.set_position((2.0, window.size().y as f32 - 14.0));
            window.draw(&version_text);

            frame_counter += 1;
            if (Instant::now() - start).as_secs_f32() >= 1.0 {
                fps_text.set_string(&format!("FPS: {frame_counter}"));
                fps_text.set_fill_color(Color::rgba(0xAF, 0xAF, 0xAF, 0xCF));
                fps_text.set_position((2.0, window.size().y as f32 - 30.0));

                frame_counter = 0;
                start = Instant::now();
            }
            window.draw(&fps_text);

            window.display();
        }
    }

    fn handle_map_io(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates) {
        let mut info_overlay = None;

        let mouse_tile_x = (self.local_mouse_x / 8.0).floor().max(0.0) as usize;
        let mouse_tile_y = (self.local_mouse_y / 8.0).floor().max(0.0) as usize;

        // Find hovered room for info overlay and possible selections
        let mut last_hovered_room_idx = None;
        for room_idx in 0..self.room_data.len() {
            if !self.plando.map().room_mask[room_idx] {
                continue;
            }

            let room_bounds = self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data);

            if !room_bounds.contains2(mouse_tile_x as i32, mouse_tile_y as i32) {
                continue;
            }

            let room_geometry = &self.plando.game_data.room_geometry[room_idx];
            let (room_x, room_y) = self.plando.map().rooms[room_idx];
            let room_width = room_geometry.map[0].len() as i32;
            let room_height = room_geometry.map.len() as i32;
            let local_tile_x = mouse_tile_x as i32 - room_x as i32;
            let local_tile_y = mouse_tile_y as i32 - room_y as i32;
            if local_tile_x < 0 || local_tile_y < 0 || local_tile_x >= room_width || local_tile_y >= room_height
                    || room_geometry.map[local_tile_y as usize][local_tile_x as usize] == 0 {
                continue;
            }

            let room_name = self.plando.game_data.room_geometry[room_idx].name.clone();
            info_overlay = Some(room_name);
            last_hovered_room_idx = Some(room_idx);
        }

        if let Some(pt) = self.map_editor.selection_start {
            if self.mouse_state.is_button_down(mouse::Button::Left) {
                let mouse_pos = Vector2i::new(mouse_tile_x as i32, mouse_tile_y as i32);
                let size = mouse_pos - pt;
                let rect = IntRect::from_vecs(pt, size);
                let rect = utils::normalize_rect(rect);
                self.draw_room_outline(rt, states, vec![rect]);
            }
        }

        if let Some(room_idx) = last_hovered_room_idx {
            if self.map_editor.dragged_room_idx.is_empty() {
                let bbox = self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data);
                self.draw_room_outline(rt, states, vec![bbox]);
            }
        }

        if !self.map_editor.selected_room_idx.is_empty() || !self.map_editor.dragged_room_idx.is_empty() {
            let idx_list = if self.map_editor.selected_room_idx.is_empty() {
                &self.map_editor.dragged_room_idx
            } else {
                &self.map_editor.selected_room_idx
            };

            let bbox_list: Vec<IntRect> = idx_list.iter().map(
                |&idx| self.plando.map_editor.get_room_bounds(idx, &self.plando.game_data)
            ).collect();
            self.draw_room_outline(rt, states, bbox_list);
        }

        // Update dragged room positions
        let mut should_redraw = self.map_editor.move_dragged_rooms(&mut self.plando.map_editor, mouse_tile_x, mouse_tile_y, &self.plando.game_data);

        // Start and stop drags
        if self.is_mouse_public && !self.click_consumed && self.mouse_state.is_button_pressed(mouse::Button::Left) {
            self.map_editor.start_drag(self.plando.map(), last_hovered_room_idx, mouse_tile_x, mouse_tile_y, &self.plando.game_data);
            self.spoiler_type = SpoilerType::None;
        }

        if self.mouse_state.is_button_released(mouse::Button::Left) {
            should_redraw = !self.map_editor.dragged_room_idx.is_empty();

            self.map_editor.stop_drag(&mut self.plando.map_editor, mouse_tile_x, mouse_tile_y, &self.plando.game_data);

            if should_redraw {
                // Fix any potential issues that may come up such as syncing up door locks
                self.plando.update_randomizable_doors();

                let door_locks = self.plando.locked_doors.clone();
                self.plando.clear_doors();
                for door in door_locks {
                    let (room_idx, door_idx) = self.plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door.src_ptr_pair];
                    if self.plando.place_door(room_idx, door_idx, Some(door.door_type), false, true).is_err() {
                        // TODO: Potentially Notify user?
                    }
                }

                if self.settings.spoiler_auto_update {
                    if let Err(err) = self.plando.update_spoiler_data() {
                        self.modal_type = ModalType::Error(err.to_string());
                    }
                }
            }
        }

        // Draw any errors
        self.draw_map_error_overlay(rt, states);

        if should_redraw {
            self.redraw_map();
        }

        if let Some(io) = info_overlay {
            self.layout.info_overlay_builder.new_line(io, Color::WHITE);
        }
    }

    fn get_error_rects(&self, error: MapErrorType) -> Vec<IntRect> {
        match error {
            MapErrorType::DoorDisconnected(room_idx, door_idx) => {
                let (room_x, room_y) = self.plando.map().rooms[room_idx];
                let door = &self.plando.game_data.room_geometry[room_idx].doors[door_idx];
                vec![IntRect::new((room_x + door.x) as i32, (room_y + door.y) as i32, 1, 1)]
            },
            MapErrorType::AreaBounds(area, _, _) => {
                (0..self.plando.map().rooms.len()).filter(|&room_idx| {
                    self.plando.map().area[room_idx] == area
                }).map(|room_idx| {
                    self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data)
                }).collect()
            },
            MapErrorType::AreaTransitions(_) => { // TODO: Some way to visualize this?
                vec![]
            },
            MapErrorType::RoomOverlap(idx1, idx2) => {
                let bbox1 = self.plando.map_editor.get_room_bounds(idx1, &self.plando.game_data);
                let bbox2 = self.plando.map_editor.get_room_bounds(idx2, &self.plando.game_data);
                if let Some(overlap) = bbox1.intersection(&bbox2) {
                    return vec![overlap];
                }
                vec![bbox1, bbox2]
            },
            MapErrorType::MapPerArea(room_idx) => {
                vec![self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data)]
            },
            MapErrorType::MapBounds(_, _, _, _) => {
                (0..self.plando.map().rooms.len()).filter_map(|room_idx| {
                    let bbox = self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data);
                    if bbox.left + bbox.width > MapEditor::MAP_MAX_SIZE as i32 || bbox.top + bbox.height > MapEditor::MAP_MAX_SIZE as i32 {
                        return Some(bbox);
                    }
                    None
                }).collect()
            }
            MapErrorType::PhantoonMap => {
                let room_idx = self.plando.game_data.room_idx_by_ptr[&511179];
                vec![self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data)]
            },
            MapErrorType::PhantoonSave => {
                let room_idx = self.plando.game_data.room_idx_by_ptr[&511626];
                vec![self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data)]
            },
            MapErrorType::ToiletNoRoom => {
                let room_idx = self.plando.game_data.toilet_room_idx;
                vec![self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data)]
            },
            MapErrorType::ToiletMultipleRooms(room_idx1, room_idx2) => {
                let room_idx = self.plando.game_data.toilet_room_idx;
                vec![self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data),
                self.plando.map_editor.get_room_bounds(room_idx1, &self.plando.game_data),
                self.plando.map_editor.get_room_bounds(room_idx2, &self.plando.game_data)]
            },
            MapErrorType::ToiletArea(room_idx, _, _) => {
                vec![self.plando.map_editor.get_room_bounds(self.plando.game_data.toilet_room_idx, &self.plando.game_data),
                self.plando.map_editor.get_room_bounds(room_idx, &self.plando.game_data)]
            },
            MapErrorType::ToiletNoPatch(_, _, _, _) => {
                vec![self.plando.map_editor.get_room_bounds(self.plando.game_data.toilet_room_idx, &self.plando.game_data)]
            },
        }
    }

    fn draw_background_grid(&self, rt: &mut dyn RenderTarget, states: &RenderStates, tex_grid: &FBox<graphics::Texture>) {
        let trans = &states.transform;

        let tl = trans.transform_point(Vector2f::default());
        let tr = Vector2f::new(rt.size().x as f32, tl.y);
        let bl = Vector2f::new(tl.x, rt.size().y as f32);
        let br: Vector2f = rt.size().as_other();

        let vertex_array = vec![
            Vertex::with_pos_coords(tl, Vector2f::default()),
            Vertex::with_pos_coords(tr, (tr - tl) / self.view.zoom),
            Vertex::with_pos_coords(bl, (bl - tl) / self.view.zoom),
            Vertex::with_pos_coords(br, (br - tl) / self.view.zoom)
        ];

        {
            let mut states = RenderStates::DEFAULT;
            states.texture = Some(&tex_grid);

            rt.draw_primitives(&vertex_array, PrimitiveType::TRIANGLE_STRIP, &states);
        }

        let mut border = graphics::RectangleShape::with_size((MapEditor::MAP_MAX_SIZE as f32 * 8.0 + 1.0, 1.0).into());
        border.set_fill_color(Color::WHITE);
        border.set_position((0.0, MapEditor::MAP_MAX_SIZE as f32 * 8.0));
        rt.draw_with_renderstates(&border, &states);

        border.set_position((MapEditor::MAP_MAX_SIZE as f32 * 8.0, 0.0));
        border.set_size((1.0, MapEditor::MAP_MAX_SIZE as f32 * 8.0));
        rt.draw_with_renderstates(&border, &states);
    }

    fn draw_map_error_overlay(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates) {
        let mut res = None;

        let mouse_tile_x = (self.local_mouse_x / 8.0).floor() as i32;
        let mouse_tile_y = (self.local_mouse_y / 8.0).floor() as i32;

        for i in 0..self.plando.map_editor.error_list.len() {
            let rects = self.get_error_rects(self.plando.map_editor.error_list[i]);

            if rects.iter().any(|rect| rect.contains2(mouse_tile_x, mouse_tile_y)) {
                res = Some(self.plando.map_editor.error_list[i].to_string(&self.plando.game_data));
            }
            
            if !rects.is_empty() {
                let color = if self.plando.map_editor.error_list[i].is_severe() { Color::RED } else { Color::YELLOW };
                self.layout.render_selection.render(rt, states, rects, color, self.global_seconds());
            }
        }

        if let Some(io) = res {
            self.layout.info_overlay_builder.new_line(io, Color::RED);
        }
    }

    fn draw_room_outline(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates, rects: Vec<IntRect>) {
        if rects.is_empty() {
            return;
        }

        self.layout.render_selection.render(rt, states, rects, Color::CYAN, self.global_seconds());
    }

    fn draw_start_locations(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates, sidebar_selection: Option<Placeable>) {
        let (tex_helm_w, _, tex_helm) = self.user_tex_source.get_texture(Placeable::Helm as u64);
        let mut sprite_helm = graphics::Sprite::with_texture(tex_helm);
        sprite_helm.set_color(Color::rgba(0xAF, 0xAF, 0xAF, 0x5F));

        let mut should_redraw = false;

        if sidebar_selection.is_some_and(|sel| sel == Placeable::Helm) {
            for i in 0..self.plando.game_data.start_locations.len() {
                let room_idx = self.plando.room_id_to_idx(self.plando.game_data.start_locations[i].room_id);
                let (room_x, room_y) = self.plando.map().rooms[room_idx];
                let tile_x = (self.plando.game_data.start_locations[i].x / 16.0).floor();
                let tile_y = (self.plando.game_data.start_locations[i].y / 16.0).floor();

                sprite_helm.set_position(Vector2f::new(room_x as f32 + tile_x, room_y as f32 + tile_y) * 8.0);
                sprite_helm.set_scale(8.0 / tex_helm_w);

                if sprite_helm.global_bounds().contains2(self.local_mouse_x, self.local_mouse_y) {
                    sprite_helm.scale(1.2);
                    if let Some(bt) = self.mouse_state.button_clicked {
                        let mut res = Ok(());
                        if bt == mouse::Button::Left {
                            res = self.plando.place_start_location(self.plando.game_data.start_locations[i].clone());
                        } else if bt == mouse::Button::Right {
                            res = self.plando.place_start_location(Plando::get_ship_start());
                        }
                        if self.settings.spoiler_auto_update {
                            if let Err(err) = self.plando.update_spoiler_data() {
                                self.modal_type = ModalType::Error(err.to_string());
                            }
                        }
                        should_redraw = true;
                        self.click_consumed = true;
                        if let Err(err) = res {
                            self.modal_type = ModalType::Error(err.to_string());
                        }
                    }
                }

                rt.draw_with_renderstates(&sprite_helm, &states);
            }
        }
        sprite_helm.set_color(Color::WHITE);
        sprite_helm.set_scale(8.0 / tex_helm_w);

        // Draw current start location
        let room_idx = self.plando.room_id_to_idx(self.plando.start_location_data.start_location.room_id);
        let (room_x, room_y) = self.plando.map().rooms[room_idx];
        let start_tile_x = (self.plando.start_location_data.start_location.x / 16.0).floor();
        let start_tile_y = (self.plando.start_location_data.start_location.y / 16.0).floor();
        sprite_helm.set_position(Vector2f::new(room_x as f32 + start_tile_x, room_y as f32 + start_tile_y) * 8.0);
        if sidebar_selection.is_none() && sprite_helm.global_bounds().contains2(self.local_mouse_x, self.local_mouse_y) {
            sprite_helm.scale(1.2);
            if self.mouse_state.button_clicked.is_some_and(|x| x == mouse::Button::Left) {
                self.spoiler_type = SpoilerType::Hub;
                self.click_consumed = true;
            }
        }
        rt.draw_with_renderstates(&sprite_helm, &states);
        drop(sprite_helm);

        if should_redraw {
            self.redraw_map();
        }
    }

    fn draw_placed_doors(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates) {
        for door in &self.plando.locked_doors {
            if door.door_type == DoorType::Blue {
                continue;
            }

            let (room_src_idx, _door_src_idx) = self.plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door.src_ptr_pair];
            let mut room_idxs = vec![(room_src_idx, door.src_ptr_pair)];
            if let Some(&(room_dst_idx, _door_dst_idx)) = self.plando.game_data.room_and_door_idxs_by_door_ptr_pair.get(&door.dst_ptr_pair) {
                room_idxs.push((room_dst_idx, door.dst_ptr_pair));
            }
            for (room_idx, ptr_pair) in room_idxs {
                let (room_x, room_y) = self.plando.map().rooms[room_idx];
                let room_geometry = &self.plando.game_data.room_geometry[room_idx];
                let (tile_x, tile_y, dir) = room_geometry.doors.iter().find_map(|door| {
                    if door.exit_ptr == ptr_pair.0 && door.entrance_ptr == ptr_pair.1 { Some((door.x, door.y, door.direction.clone())) } else { None }
                }).expect("LockedDoor vector contains non-existent door");
                let x = ((room_x + tile_x) * 8) as f32;
                let y = ((room_y + tile_y) * 8) as f32;

                let door_tex_id = match door.door_type {
                    DoorType::Red => Placeable::DoorMissile,
                    DoorType::Green => Placeable::DoorSuper,
                    DoorType::Yellow => Placeable::DoorPowerBomb,
                    DoorType::Beam(beam_type) => match beam_type {
                        BeamType::Charge => Placeable::DoorCharge,
                        BeamType::Ice => Placeable::DoorIce,
                        BeamType::Wave => Placeable::DoorWave,
                        BeamType::Spazer => Placeable::DoorSpazer,
                        BeamType::Plasma => Placeable::DoorPlasma,
                    },
                    DoorType::Wall => Placeable::DoorWall,
                    _ => Placeable::DoorMissile
                } as u64;
                let (_tex_w, _tex_h, door_tex) = self.user_tex_source.get_texture(door_tex_id);
                let mut door_spr = graphics::Sprite::with_texture(door_tex);
                door_spr.set_origin((4.0, 4.0));
                door_spr.set_position((x + 4.0, y + 4.0));
                door_spr.set_rotation(match dir.as_str() {
                    "up" => 90.0,
                    "right" => 180.0,
                    "down" => 270.0,
                    _ => 0.0
                });

                rt.draw_with_renderstates(&door_spr, &states);
            }
        }
    }

    fn draw_gray_doors(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates) {
        for door_ptr_pair in &self.plando.gray_doors {
            let (room_idx, door_idx) = self.plando.game_data.room_and_door_idxs_by_door_ptr_pair[door_ptr_pair];
            if !self.plando.map().room_mask[room_idx] {
                continue;
            }

            let (room_x, room_y) = self.plando.map().rooms[room_idx];
            let room_geomtry = &self.plando.game_data.room_geometry[room_idx];
            let door = &room_geomtry.doors[door_idx];
            let x = (room_x + door.x) as f32 * 8.0;
            let y = (room_y + door.y) as f32 * 8.0;

            let tex = self.user_tex_source.get_texture(UserTexId::DoorGray as u64).2;
            let mut door_spr = graphics::Sprite::with_texture(tex);
            door_spr.set_origin((4.0, 4.0));
            door_spr.set_position((x + 4.0, y + 4.0));
            door_spr.set_rotation(match door.direction.as_str() {
                "up" => 90.0,
                "right" => 180.0,
                "down" => 270.0,
                _ => 0.0
            });

            rt.draw_with_renderstates(&door_spr, &states);
        }
    }

    fn draw_items(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates, sidebar_selection: Option<Placeable>, tex_items: &FBox<graphics::Texture>) {
        let mut info_overlay = None;
        if sidebar_selection.is_none() || sidebar_selection.is_some_and(|x| x >= Placeable::ETank) {
            let tex_item_width = tex_items.size().x as i32 / 24;
            for i in 0..self.plando.item_locations.len() {
                let item = self.plando.item_locations[i];
                let (room_id, node_id) = self.plando.game_data.item_locations[i];
                let room_ptr = self.plando.game_data.room_ptr_by_id[&room_id];
                let room_idx = self.plando.game_data.room_idx_by_ptr[&room_ptr];
                if !self.plando.map().room_mask[room_idx] {
                    continue;
                }

                let (room_x, room_y) = self.plando.map().rooms[room_idx];
                let (tile_x, tile_y) = self.plando.game_data.node_coords[&(room_id, node_id)];

                let item_index = match item {
                    Item::Nothing => 23,
                    item => item as i32
                };

                let mut spr_item = graphics::Sprite::with_texture_and_rect(&tex_items,
                    IntRect::new(tex_item_width * item_index, 0, tex_item_width, tex_item_width));
                spr_item.set_origin(Vector2f::new(tex_item_width as f32 / 2.0, tex_item_width as f32 / 2.0));
                let double_item = get_double_item_offset(room_id, node_id);
                let item_x_offset = match double_item {
                    DoubleItemPlacement::Left => 2,
                    DoubleItemPlacement::Middle => 4,
                    DoubleItemPlacement::Right => 6
                };
                spr_item.set_position(Vector2f::new((8 * (tile_x + room_x) + item_x_offset) as f32, (8 * (tile_y + room_y) + 4) as f32));
                spr_item.set_scale(6.0 / tex_item_width as f32);
                if spr_item.global_bounds().contains2(self.local_mouse_x, self.local_mouse_y) && self.is_mouse_public {
                    spr_item.scale(1.2);
                    let item_name = if item_index == 23 {
                        &"Nothing".to_string()
                    } else {
                        &self.plando.game_data.item_isv.keys[item_index as usize]
                    };
                    info_overlay = Some(item_name.clone());

                    if let Some(bt) = self.mouse_state.button_clicked {
                        if bt == mouse::Button::Left {
                            if sidebar_selection.is_some_and(|x| x.to_item().is_some()) {
                                let item_to_place = sidebar_selection.unwrap().to_item().unwrap();
                                let as_placeable = Placeable::from_item(item_to_place);
                                let max_count = self.plando.get_max_placeable_count(as_placeable).unwrap_or(999);
                                if self.plando.placed_item_count[as_placeable as usize] < max_count {
                                    self.plando.place_item(i, item_to_place);
                                    if self.settings.spoiler_auto_update {
                                        if let Err(err) = self.plando.update_spoiler_data() {
                                            self.modal_type = ModalType::Error(err.to_string());
                                        }
                                    }
                                    self.redraw_map();
                                }
                            } else {
                                self.spoiler_type = SpoilerType::Item(i);
                            }
                        } else if bt == mouse::Button::Right {
                            self.plando.place_item(i, Item::Nothing);
                            if self.settings.spoiler_auto_update {
                                if let Err(err) = self.plando.update_spoiler_data() {
                                    self.modal_type = ModalType::Error(err.to_string());
                                }
                            }
                            self.redraw_map();
                        }
                        self.click_consumed = true;
                    }

                    self.is_mouse_public = false;
                }

                rt.draw_with_renderstates(&spr_item, &states);
            }
        }

        if let Some(io) = info_overlay {
            self.layout.info_overlay_builder.new_line(io, Color::WHITE);
        }
    }

    fn draw_flags(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates, sidebar_selection: Option<Placeable>) {
        let mut info_overlay = None;
        for (i, &flag_id) in self.plando.game_data.flag_ids.iter().enumerate() {
            let vertex_info = &self.plando.get_vertex_info(self.plando.game_data.flag_vertex_ids[i][0]);
            let room_idx = self.plando.room_id_to_idx(vertex_info.room_id);
            if !self.plando.map().room_mask[room_idx] {
                continue;
            }

            let (room_x, room_y) = self.plando.map().rooms[room_idx];

            let flag_str_short = &self.plando.game_data.flag_isv.keys[flag_id];
            let flag_info = get_flag_info(flag_str_short);
            if flag_info.is_err() {
                continue;
            }
            let (flag_x, flag_y, flag_type, flag_str) = flag_info.unwrap();
            let flag_position = Vector2f::new(room_x as f32 + flag_x + 0.5, room_y as f32 + flag_y + 0.5);
            let (tex_w, tex_h, tex) = self.user_tex_source.get_texture(flag_type as u64);
            let mut spr_flag = graphics::Sprite::with_texture(tex);
            spr_flag.set_origin(Vector2f::new(tex_w / 2.0, tex_h / 2.0));
            spr_flag.set_position(flag_position * 8.0);
            spr_flag.set_scale(8.0 / tex_w);

            if spr_flag.global_bounds().contains2(self.local_mouse_x, self.local_mouse_y) && self.is_mouse_public {
                spr_flag.scale(1.3);

                info_overlay = Some(flag_str.to_string());

                if sidebar_selection.is_none() && self.mouse_state.button_clicked.is_some_and(|bt| bt == mouse::Button::Left) {
                    self.spoiler_type = SpoilerType::Flag(i);
                    self.click_consumed = true;
                }

                self.is_mouse_public = false;
            }

            rt.draw_with_renderstates(&spr_flag, &states);
        }

        if let Some(io) = info_overlay {
            self.layout.info_overlay_builder.new_line(io, Color::WHITE);
        }
    }

    fn draw_door_hover(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates, sidebar_selection: Option<Placeable>) {
        let tile_x = (self.local_mouse_x / 8.0).floor().max(0.0) as usize;
        let tile_y = (self.local_mouse_y / 8.0).floor().max(0.0) as usize;
        
        let room_idx = match self.plando.map_editor.get_room_at(tile_x, tile_y, &self.plando.game_data) {
            None => return,
            Some(room_idx) => room_idx
        };

        let mut should_redraw = false;

        if sidebar_selection.is_some_and(|x| x.to_door_type().is_some()) && self.is_mouse_public {
            let door_type = sidebar_selection.unwrap();
            let tr = (self.local_mouse_x / 8.0).fract() > (self.local_mouse_y / 8.0).fract();
            let br = (self.local_mouse_x / 8.0).fract() > 1.0 - (self.local_mouse_y / 8.0).fract();
            let direction = (if tr && br { "right" } else if tr && !br { "up" } else if !tr && br { "down" } else { "left" }).to_string();

            let (room_x, room_y) = self.plando.map().rooms[room_idx];
            let tile_x = tile_x - room_x;
            let tile_y = tile_y - room_y;
            let door_idx_opt = self.plando.game_data.room_geometry[room_idx].doors.iter().position(
                |x| x.direction == direction && x.x == tile_x && x.y == tile_y
            );
            if let Some(door_idx) = door_idx_opt {
                let (room_x, room_y) = self.plando.map().rooms[room_idx];
                let x = (room_x + tile_x) as f32;
                let y = (room_y + tile_y) as f32;

                let (_tex_w, _tex_h, tex) = self.user_tex_source.get_texture(door_type as u64);
                let mut spr_ghost = graphics::Sprite::with_texture(tex);
                spr_ghost.set_position(Vector2f::new(x, y) * 8.0);
                spr_ghost.move_(4.0);
                spr_ghost.set_origin(4.0);
                spr_ghost.set_rotation(match direction.as_str() {
                    "up" => 90.0,
                    "right" => 180.0,
                    "down" => 270.0,
                    _ => 0.0
                });
                spr_ghost.set_color(Color::rgba(0xFF, 0xFF, 0xFF, 0x7F));

                if let Some(bt) = self.mouse_state.button_clicked {
                    let mut res = Ok(());
                    if bt == mouse::Button::Left {
                        res = self.plando.place_door(room_idx, door_idx, door_type.to_door_type(), false, false);
                    } else if bt == mouse::Button::Right {
                        res = self.plando.place_door(room_idx, door_idx, None, true, false);
                    }
                    if self.settings.spoiler_auto_update {
                        if let Err(err) = self.plando.update_spoiler_data() {
                            self.modal_type = ModalType::Error(err.to_string());
                        }
                    }
                    should_redraw = true;
                    self.click_consumed = true;
                    if let Err(err) = res {
                        self.modal_type = ModalType::Error(err.to_string());
                    }
                }

                rt.draw_with_renderstates(&spr_ghost, &states);
            }
        }

        if should_redraw {
            self.redraw_map();
        }
    }

    fn draw_spoiler_route(&mut self, rt: &mut dyn RenderTarget, states: &RenderStates) {
        if self.spoiler_type != SpoilerType::None && self.plando.randomization.is_some() {
            let (_r, spoiler_log) = self.plando.randomization.as_ref().unwrap();
            let mut obtain_route = None;
            let mut return_route = None;
            let mut show_escape_route = false;

            match self.spoiler_type {
                SpoilerType::Hub => {
                    obtain_route = Some(&spoiler_log.hub_obtain_route);
                    return_route = Some(&spoiler_log.hub_return_route);
                    self.spoiler_step = 0;
                }
                SpoilerType::Item(spoiler_idx) => {
                    let (room_id, node_id) = self.plando.game_data.item_locations[spoiler_idx];
                    let mut details_opt = None;
                    while details_opt.is_none() {
                        if self.spoiler_step < spoiler_log.details.len() {
                            details_opt = spoiler_log.details[self.spoiler_step].items.iter().find(
                                |x| x.location.room_id == room_id && x.location.node_id == node_id
                            );
                        }
                        if details_opt.is_none() {
                            let step_opt = spoiler_log.details.iter().position(
                                |x| x.items.iter().any(|y| y.location.room_id == room_id && y.location.node_id == node_id)
                            );
                            if step_opt.is_none() {
                                break;
                            }
                            self.spoiler_step = step_opt.unwrap();
                        }
                    }
                    if let Some(details) = details_opt {
                        obtain_route = Some(&details.obtain_route);
                        return_route = Some(&details.return_route);
                    } else {
                        self.modal_type = ModalType::Error("Item is nothing or not logically bireachable".to_string());
                        self.spoiler_type = SpoilerType::None;
                    }
                }
                SpoilerType::Flag(spoiler_idx) => {
                    let flag_id = self.plando.game_data.flag_ids[spoiler_idx];
                    let flag_name = &self.plando.game_data.flag_isv.keys[flag_id];
                    
                    let mut details_opt = None;
                    while details_opt.is_none() {
                        if self.spoiler_step < spoiler_log.details.len() {
                            details_opt = spoiler_log.details[self.spoiler_step].flags.iter().find(
                                |x| x.flag == *flag_name
                            );
                        }
                        if details_opt.is_none() {
                            let step_opt = spoiler_log.details.iter().position(
                                |x| x.flags.iter().any(|y| y.flag == *flag_name)
                            );
                            if step_opt.is_none() {
                                break;
                            }
                            self.spoiler_step = step_opt.unwrap();
                        }
                    }
                    if let Some(details) = details_opt {
                        obtain_route = Some(&details.obtain_route);
                        return_route = Some(&details.return_route);
                        show_escape_route = flag_id == self.plando.game_data.mother_brain_defeated_flag_id;
                    } else {
                        self.modal_type = ModalType::Error("Flag not logically clearable".to_string());
                        self.spoiler_type = SpoilerType::None;
                    }
                }
                _ => {
                    self.spoiler_step = spoiler_log.details.len();
                }
            }

            if obtain_route.is_some() && return_route.is_some() {
                let mut vertex_return = Vec::new();
                let mut vertex_obtain = Vec::new();
                let mut vertex_escape = Vec::new();
                for entry in return_route.unwrap() {
                    if let Some((x, y)) = entry.coords {
                        let vertex = graphics::Vertex::with_pos_color(Vector2f::new(x as f32 + 0.5, y as f32 + 0.5) * 8.0, Color::YELLOW);
                        vertex_return.push(vertex);
                    }
                }
                for entry in obtain_route.unwrap() {
                    if let Some((x, y)) = entry.coords {
                        let vertex = graphics::Vertex::with_pos_color(Vector2f::new(x as f32 + 0.5, y as f32 + 0.5) * 8.0, Color::WHITE);
                        vertex_obtain.push(vertex);
                    }
                }
                if show_escape_route {
                    if let Some(animal_route) = spoiler_log.escape.animals_route.as_ref() {
                        for entry in animal_route {
                            let v1 = graphics::Vertex::with_pos_color(Vector2f::new(entry.from.x as f32 + 0.5, entry.from.y as f32 + 0.5) * 8.0, Color::CYAN);
                            let v2 = graphics::Vertex::with_pos_color(Vector2f::new(entry.to.x as f32 + 0.5, entry.to.y as f32 + 0.5) * 8.0, Color::CYAN);
                            vertex_escape.push(v1);
                            vertex_escape.push(v2);
                        }
                    }
                    for entry in &spoiler_log.escape.ship_route {
                        let v1 = graphics::Vertex::with_pos_color(Vector2f::new(entry.from.x as f32 + 0.5, entry.from.y as f32 + 0.5) * 8.0, Color::CYAN);
                        let v2 = graphics::Vertex::with_pos_color(Vector2f::new(entry.to.x as f32 + 0.5, entry.to.y as f32 + 0.5) * 8.0, Color::CYAN);
                        vertex_escape.push(v1);
                        vertex_escape.push(v2);
                    }
                }

                draw_thick_line_strip(rt, &states, &vertex_escape, 1.0);
                draw_thick_line_strip(rt, &states, &vertex_return, 1.0);
                draw_thick_line_strip(rt, &states, &vertex_obtain, 1.0);
            }
        }
    }

    fn draw_sidebar(&mut self, ui: &mut Ui, sidebar_selection: &mut Option<Placeable>) {
        ui.scope(|ui| {
            ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;

            ui.horizontal(|ui| {
                let sel_color = ui.style().visuals.panel_fill;

                for tab in SidebarPanel::VARIANTS {
                    let title = match tab {
                        SidebarPanel::Items => "Items".to_string(),
                        SidebarPanel::Rooms => "Rooms".to_string(),
                        SidebarPanel::Areas => "Areas".to_string(),
                        SidebarPanel::Errors => format!("Errors ({})", self.plando.map_editor.error_list.len()),
                    };

                    let mut bt = egui::Button::new(title)
                        .corner_radius(egui::CornerRadius { se: 0, sw: 0, ne: 4, nw: 4 });

                    if *tab != self.layout.sidebar_tab {
                        bt = bt.fill(sel_color);
                    }

                    if ui.add(bt).clicked() {
                        self.layout.sidebar_tab = tab.clone();
                        self.redraw_map();
                    }
                }
            });

            ui.add(egui::Separator::default().spacing(1.0));
        });

        match self.layout.sidebar_tab {
            SidebarPanel::Items => self.draw_sidebar_item_select(ui, sidebar_selection),
            SidebarPanel::Rooms => self.draw_sidebar_room_select(ui),
            SidebarPanel::Areas => self.draw_sidebar_area_select(ui),
            SidebarPanel::Errors => self.draw_sidebar_error_list(ui)
        }
    }

    fn draw_sidebar_item_select(&mut self, ui: &mut Ui, sidebar_selection: &mut Option<Placeable>) {
        let sidebar_height = 32.0;
        let cur_sidebar_selection = sidebar_selection.clone();

        egui::scroll_area::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            egui::Grid::new("grid_item_select")
            .with_row_color(move |val, _style| {
                if cur_sidebar_selection.is_some_and(|x| x == Placeable::VARIANTS[val]) { Some(Color32::from_rgb(255, 0, 0)) } else { None }
            }).min_row_height(sidebar_height).show(ui, |ui| {
                for (row, placeable) in Placeable::VARIANTS.iter().enumerate() {
                    // Load image
                    let img = egui::Image::new(self.user_tex_source.get_image_source(*placeable as u64)).sense(Sense::click())
                        .fit_to_exact_size(Vec2::new(sidebar_height, sidebar_height));
                    let img_resp = ui.add(img);
                    if img_resp.clicked() {
                        *sidebar_selection = Some(Placeable::VARIANTS[row]);
                    }

                    let item_count = self.plando.placed_item_count[row];
                    let max_item_count = self.plando.get_max_placeable_count(placeable.clone());

                    let label_name = egui::Label::new(placeable.to_string());
                    if ui.add(label_name).clicked() {
                        *sidebar_selection = Some(Placeable::VARIANTS[row]);
                    }
                    let label_count_str = if max_item_count.is_some() { format!("{item_count} / {}", max_item_count.unwrap()) } else { item_count.to_string() };
                    let label_count = egui::Label::new(label_count_str).sense(Sense::click());
                    if ui.add(label_count).clicked() {
                        *sidebar_selection = Some(Placeable::VARIANTS[row]);
                    }
                    // So it doesn't create an empty row at the very end
                    if row + 1 < Placeable::VARIANTS.len() {
                        ui.end_row();
                    }
                }
            });
        });
    }

    fn draw_sidebar_room_select(&mut self, ui: &mut Ui) {
        ui.text_edit_singleline(&mut self.room_search.name);
        ui.collapsing("Advanced Search", |ui| {
            egui::ComboBox::from_label("Heated").selected_text(self.room_search.is_heated.to_string()).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.room_search.is_heated, SearchOpt::Any, "Any");
                ui.selectable_value(&mut self.room_search.is_heated, SearchOpt::Yes, "Yes");
                ui.selectable_value(&mut self.room_search.is_heated, SearchOpt::No, "No");
            });

            egui::Grid::new("grid_adv_search").striped(true).num_columns(3).show(ui, |ui| {
                ui.label("");
                ui.label("Min");
                ui.label("Max");
                ui.end_row();

                ui.label("Width");
                ui.add(egui::DragValue::new(&mut self.room_search.min_width).range(0..=self.room_search.max_width));
                ui.add(egui::DragValue::new(&mut self.room_search.max_width).range(self.room_search.min_width..=99));
                ui.end_row();
                
                ui.label("Height");
                ui.add(egui::DragValue::new(&mut self.room_search.min_height).range(0..=self.room_search.max_height));
                ui.add(egui::DragValue::new(&mut self.room_search.max_height).range(self.room_search.min_height..=99));
                ui.end_row();

                let door_order = ["Right", "Down", "Left", "Up"];
                for i in 0..4 {
                    ui.label(format!("{} Door", door_order[i]));
                    ui.add(egui::DragValue::new(&mut self.room_search.min_door_count[i]).range(0..=self.room_search.max_door_count[i]));
                    ui.add(egui::DragValue::new(&mut self.room_search.max_door_count[i]).range(self.room_search.min_door_count[i]..=9));
                    ui.end_row();
                }
            });
        });

        if ui.button("Clear filters").clicked() {
            self.room_search.reset();
        }

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let room_idxs = self.room_search.filter(&self.plando.game_data);
            for room_idx in room_idxs {
                let is_missing = !self.plando.map().room_mask[room_idx];
                let room_geometry = &self.plando.game_data.room_geometry[room_idx];
                let room_name = &room_geometry.name;
                let mut btn = egui::Button::new(room_name).min_size(Vec2 { x: 256.0, y: 1.0 });
                if is_missing {
                    btn = btn.fill(Color32::RED);
                }
                if ui.add(btn).clicked() {
                    if is_missing {
                        self.plando.map_editor.spawn_room(room_idx, &self.plando.game_data);
                    }
                    let (room_x, room_y) = self.plando.map().rooms[room_idx];
                    let room_width = room_geometry.map[0].len() as f32 * 8.0;
                    let room_height = room_geometry.map.len() as f32 * 8.0;
                    let room_rect = FloatRect::new(room_x as f32 * 8.0, room_y as f32 * 8.0, room_width, room_height);
                    self.view.focus_rect(room_rect);

                    self.map_editor.selected_room_idx.clear();
                    self.map_editor.selected_room_idx.push(room_idx);
                    self.redraw_map();
                }
            }
        });
    }

    fn draw_sidebar_area_select(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let text_color = ui.style().visuals.strong_text_color();
            let inv_color = Color32::from_rgb(255 - text_color.r(), 255 - text_color.g(), 255 - text_color.b());
            let should_inv = |col: Color32| {
                utils::rgb_to_hsv(col.r(), col.g(), col.b()).2 >= 0.5
            };

            ui.label("Apply area to room selection");
            ui.separator();
            ui.label("Major Areas (retains subarea values)");

            let areas = ["Crateria", "Brinstar", "Norfair", "Wrecked Ship", "Maridia", "Tourian"];
            for (idx, &area_str) in areas.iter().enumerate() {
                let col = map_editor::Area::from_tuple((idx, 0, 0)).to_color();
                let col32 = Color32::from_rgb(col.r, col.g, col.b);
                let stroke_col = if should_inv(col32) { &inv_color } else { &text_color };

                let btn = egui::Button::new(RichText::new(area_str).color(stroke_col.clone())).fill(col32).min_size(Vec2 { x: 256.0, y: 1.0 });
                if ui.add(btn).clicked() && !self.map_editor.selected_room_idx.is_empty() {
                    for i in 0..self.map_editor.selected_room_idx.len() {
                        let room_idx = self.map_editor.selected_room_idx[i];
                        let sub_area = self.plando.map().subarea[room_idx];
                        let sub_sub_area = self.plando.map().subsubarea[room_idx];
                        self.plando.map_editor.apply_area(room_idx, map_editor::Area::from_tuple((idx, sub_area, sub_sub_area)));
                    }
                    self.redraw_map();
                }
            }
            ui.separator();
            ui.label("Area and subareas (mainly affects music)");

            for area_value in map_editor::Area::VALUES {
                let col = area_value.to_color();
                let col32 = Color32::from_rgb(col.r, col.g, col.b);
                let stroke_col = if should_inv(col32) { &inv_color } else { &text_color };

                let btn = egui::Button::new(RichText::new(area_value.to_string()).color(stroke_col.clone())).fill(col32).min_size(Vec2 { x: 256.0, y: 1.0 });
                if ui.add(btn).clicked() && !self.map_editor.selected_room_idx.is_empty() {
                    for i in 0..self.map_editor.selected_room_idx.len() {
                        self.plando.map_editor.apply_area(self.map_editor.selected_room_idx[i], area_value);
                    }
                    self.redraw_map();
                }
            }
            ui.separator();

            ui.label("Swap Areas:");
            egui::ComboBox::from_id_salt("combo_swap_area_first").selected_text(areas[self.map_editor.swap_first]).show_ui(ui, |ui| {
                for (idx, &area_str) in areas.iter().enumerate() {
                    ui.selectable_value(&mut self.map_editor.swap_first, idx, area_str);
                }
            });
            if ui.button("Swap!").clicked() {
                self.plando.map_editor.swap_areas(self.map_editor.swap_first, self.map_editor.swap_second);
                self.redraw_map();
            }
            egui::ComboBox::from_id_salt("combo_swap_area_second").selected_text(areas[self.map_editor.swap_second]).show_ui(ui, |ui| {
                for (idx, &area_str) in areas.iter().enumerate() {
                    ui.selectable_value(&mut self.map_editor.swap_second, idx, area_str);
                }
            });
        });
    }

    fn draw_sidebar_error_list(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let w = ui.available_width();

            for error in &self.plando.map_editor.error_list {
                let bt = egui::Button::new(error.to_string(&self.plando.game_data)).fill(
                    if error.is_severe() { Color32::from_rgb(110, 24, 24) } else { Color32::from_rgb(84, 80, 0) }
                ).min_size(Vec2 { x: w, y: 1.0 });
                if ui.add(bt).clicked() {
                    let rects = self.get_error_rects(error.clone());
                    if !rects.is_empty() {
                        let bbox = rects.into_iter().reduce(|acc, e| {
                            let acc_right = acc.left + acc.width;
                            let acc_bottom = acc.top + acc.height;
                            let e_right = e.left + e.width;
                            let e_bottom = e.top + e.height;

                            let new_left = acc.left.min(e.left);
                            let new_top = acc.top.min(e.top);
                            let new_right = acc_right.max(e_right);
                            let new_bottom = acc_bottom.max(e_bottom);
                            IntRect::new(new_left, new_top, new_right - new_left, new_bottom - new_top)
                        }).unwrap();
                        let scaled_bbox = IntRect::new(bbox.left * 8, bbox.top * 8, bbox.width * 8, bbox.height * 8);

                        self.view.focus_rect(scaled_bbox.as_other());
                    }
                }
            }
        });
    }

    fn draw_spoiler_override(&mut self, ctx: &Context) -> bool {
        let mut hovered = false;
        let step = self.override_window.unwrap();
        let window = egui::Window::new(format!("Spoiler Overrides STEP {}", step)).resizable(false).show(ctx, |ui| {
            let mut remove_idx = None;
            let overrides: Vec<_> = self.plando.spoiler_overrides.iter_mut().enumerate().filter(|(_idx, x)| x.step == step).collect();
            for (override_idx, item_override) in overrides {
                ui.horizontal(|ui| {
                    let cur_item = self.plando.item_locations[item_override.item_idx];
                    let cur_item_name = Placeable::from_item(cur_item).to_string();
                    hovered |= egui::ComboBox::new(format!("combo_spoiler_override_item_{override_idx}"), "Item").selected_text(&cur_item_name).show_ui(ui, |ui| {
                        for item in ITEM_VALUES {
                            let locs: Vec<_> = self.plando.item_locations.iter().enumerate().filter(
                                |&(_idx, new_item)| item == *new_item
                            ).collect();
                            if locs.is_empty() || item == Item::Nothing {
                                continue;
                            }
                            let item_name = Placeable::from_item(item).to_string();
                            if ui.selectable_label(item_name == cur_item_name, item_name).clicked() {
                                item_override.item_idx = locs[0].0;
                            }
                        }
                    }).response.contains_pointer();
                    let cur_item = self.plando.item_locations[item_override.item_idx];
                    let locs: Vec<_> = self.plando.item_locations.iter().enumerate().filter(
                        |&(_idx, new_item)| cur_item == *new_item
                    ).collect();
                    let loc_strs: Vec<_> = locs.iter().map(|&(idx, _item)| {
                        let vertex_idx = self.plando.game_data.item_vertex_ids[idx][0];
                        let vertex = &self.plando.game_data.vertex_isv.keys[vertex_idx];
                        let room_name = self.plando.game_data.room_json_map[&vertex.room_id]["name"].as_str().unwrap().to_string();
                        let node_name = self.plando.game_data.node_json_map[&(vertex.room_id, vertex.node_id)]["name"].as_str().unwrap().to_string();
                        format!("{}: {}", room_name, node_name)
                    }).collect();
                    let cur_loc_idx = locs.iter().position(|&(idx, _item)| idx == item_override.item_idx).unwrap();
                    let loc_str = &loc_strs[cur_loc_idx];
                    
                    hovered |= egui::ComboBox::new(format!("combo_spoiler_override_loc_{override_idx}"), "Location").selected_text(loc_str).show_ui(ui, |ui| {
                        for (i, &(idx, _item)) in locs.iter().enumerate() {
                            ui.selectable_value(&mut item_override.item_idx, idx, &loc_strs[i]);
                        }
                    }).response.contains_pointer();

                    if ui.button("Delete").clicked() {
                        remove_idx = Some(override_idx);
                    }
                });

                ui.label("Description");
                ui.text_edit_multiline(&mut item_override.description);
                ui.separator();
            }

            if let Some(idx) = remove_idx {
                self.plando.spoiler_overrides.remove(idx);
            }

            ui.horizontal(|ui| {
                if ui.button("New").clicked() {
                    match self.plando.item_locations.iter().position(|&item| item != Item::Nothing) {
                        None => self.modal_type = ModalType::Error("There needs to be at least one item placed to configure overrides".to_string()),
                        Some(item_idx) => self.plando.spoiler_overrides.push(SpoilerOverride {
                            step,
                            item_idx,
                            description: String::new()
                        })
                    }
                }
                if ui.button("Apply").clicked() {
                    self.override_window = None;
                    if self.settings.spoiler_auto_update {
                        self.plando.update_spoiler_data();
                    }
                }
            });
            
        }).unwrap().response;
        hovered || window.contains_pointer()
    }

    fn draw_spoiler_details(&mut self, ctx: &Context) -> bool {
        let (_r, spoiler_log) = self.plando.randomization.as_ref().unwrap();
        let window = egui::Window::new("Spoiler Details")
            .resizable(false)
            .title_bar(false)
            .movable(false)
            .vscroll(false)
            .min_width(360.0 * self.settings.ui_scale)
            .max_width(720.0 * self.settings.ui_scale)
            .fixed_pos(Vec2::new(16.0, 32.0).to_pos2())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.style_mut().spacing.item_spacing = Vec2::new(2.0, 2.0);
                    let details = &spoiler_log.details[self.spoiler_step];
                    ui.horizontal(|ui| {
                        ui.heading(format!("STEP {}", details.step));
                        if ui.button("Modify Overrides").clicked() {
                            self.override_window = Some(details.step);
                        }
                    });
                    ui.label("PREVIOUSLY COLLECTIBLE");

                    let mut collectible_items = [0; ITEM_VALUES.len() - 1];
                    let mut collectible_flags = vec![false; self.plando.game_data.flag_ids.len()];
                    for prev_step in 0..self.spoiler_step {
                        let prev_details = &spoiler_log.details[prev_step];
                        for item_details in &prev_details.items {
                            let item_id = self.plando.game_data.item_isv.index_by_key[&item_details.item];
                            collectible_items[item_id] += 1;
                        }
                        for flag_details in &prev_details.flags {
                            let flag_id = self.plando.game_data.flag_isv.index_by_key[&flag_details.flag];
                            let flag_idx = self.plando.game_data.flag_ids.iter().position(|x| *x == flag_id).unwrap();
                            collectible_flags[flag_idx] = true;
                        }
                    }

                    let mut new_spoiler_type = self.spoiler_type.clone();

                    let minor_indices = [Item::Missile as usize, Item::Super as usize, Item::PowerBomb as usize, Item::ETank as usize, Item::ReserveTank as usize];
                    // Render Minor Items
                    ui.horizontal(|ui| {
                        for i in 0..minor_indices.len() {
                            let idx = minor_indices[i];
                            if collectible_items[idx] == 0 {
                                continue;
                            }
                            let placeable_idx = Placeable::ETank as u64 + idx as u64;
                            let img = egui::Image::new(self.user_tex_source.get_image_source(placeable_idx)).fit_to_exact_size(Vec2::new(16.0, 16.0) * self.settings.ui_scale);
                            ui.add(img);
                            let ammo_collected = (collectible_items[idx] as f32 * self.plando.randomizer_settings.item_progression_settings.ammo_collect_fraction).round() as i32 * 5;
                            let label = if idx == Item::ETank as usize || idx == Item::ReserveTank as usize {
                                collectible_items[idx].to_string()
                            } else {
                                ammo_collected.to_string() + " / " + &(collectible_items[idx] * 5).to_string()
                            };
                            ui.label(label);
                        }
                    });
                    // Render Major Items
                    ui.horizontal(|ui| {
                        for i in 0..ITEM_VALUES.len() - 1 {
                            if collectible_items[i] == 0 || minor_indices.contains(&i) {
                                continue;
                            }
                            let placeable_idx = Placeable::ETank as u64 + i as u64;
                            let img = egui::Image::new(self.user_tex_source.get_image_source(placeable_idx)).fit_to_exact_size(Vec2::new(16.0, 16.0) * self.settings.ui_scale);
                            ui.add(img);
                        }
                    });
                    // Render Flags
                    ui.horizontal(|ui| {
                        for i in 0..collectible_flags.len() {
                            let flag_id = self.plando.game_data.flag_ids[i];
                            if !collectible_flags[i] || !self.flag_has_tex.contains(&flag_id) {
                                continue;
                            }
                            let flag_tex_idx = UserTexId::FlagFirst as u64 + flag_id as u64;
                            let img = egui::Image::new(self.user_tex_source.get_image_source(flag_tex_idx)).fit_to_exact_size(Vec2::new(24.0, 24.0) * self.settings.ui_scale).sense(Sense::click());
                            let resp = ui.add(img);
                            if resp.clicked() {
                                new_spoiler_type = SpoilerType::Flag(i);
                            }
                        }
                    });

                    ui.label("COLLECTIBLE ON THIS STEP");
                    // Items
                    ui.horizontal_wrapped(|ui| {
                        let mut idxs: Vec<usize> = (0..details.items.len()).collect();
                        idxs.sort_by(|&a, &b| {
                            let item_a = self.plando.game_data.item_isv.index_by_key[&details.items[a].item];
                            let item_b = self.plando.game_data.item_isv.index_by_key[&details.items[b].item];
                            item_a.cmp(&item_b)
                        });
                        for i in idxs {
                            let item_details = &details.items[i];
                            let item_id = self.plando.game_data.item_isv.index_by_key[&item_details.item];
                            let placeable_id = Placeable::ETank as u64 + item_id as u64;
                            let img = egui::Image::new(self.user_tex_source.get_image_source(placeable_id)).fit_to_exact_size(Vec2::new(16.0, 16.0) * self.settings.ui_scale).sense(Sense::click());
                            if ui.add(img).clicked() {
                                let item_idx = self.plando.game_data.item_locations.iter().position(
                                    |x| x.0 == item_details.location.room_id && x.1 == item_details.location.node_id
                                ).unwrap();
                                new_spoiler_type = SpoilerType::Item(item_idx);
                            }
                        }
                    });
                    // Flags
                    ui.horizontal_wrapped(|ui| {
                        for flag_details in &details.flags {
                            let flag_id = self.plando.game_data.flag_isv.index_by_key[&flag_details.flag];
                            if !self.flag_has_tex.contains(&flag_id) {
                                continue;
                            }
                            let flag_tex_id = UserTexId::FlagFirst as u64 + flag_id as u64;
                            let img = egui::Image::new(self.user_tex_source.get_image_source(flag_tex_id)).fit_to_exact_size(Vec2::new(24.0, 24.0) * self.settings.ui_scale).sense(Sense::click());
                            if ui.add(img).clicked() {
                                let flag_idx = self.plando.game_data.flag_ids.iter().position(
                                    |x| *x == flag_id
                                ).unwrap();
                                new_spoiler_type = SpoilerType::Flag(flag_idx);
                            }
                        }
                    });

                    let details_name: String;
                    let details_location: String;
                    let details_area: String;
                    let details_obtain_route: &Vec<SpoilerRouteEntry>;
                    let details_return_route: &Vec<SpoilerRouteEntry>;

                    match self.spoiler_type {
                        SpoilerType::Item(item) => {
                            let (room_id, node_id) = self.plando.game_data.item_locations[item];
                            let item_details = details.items.iter().find(
                                |x| x.location.room_id == room_id && x.location.node_id == node_id
                            ).unwrap();
                            details_name = match item_details.difficulty.as_ref() {
                                None => item_details.item.clone(),
                                Some(diff) => format!("{} ({})", item_details.item.clone(), diff)    
                            };
                            details_location = item_details.location.room.clone() + ": " + &item_details.location.node;
                            details_area = item_details.location.area.clone();
                            details_obtain_route = &item_details.obtain_route;
                            details_return_route = &item_details.return_route;
                        }
                        SpoilerType::Flag(flag_idx) => {
                            let flag_id = self.plando.game_data.flag_ids[flag_idx];
                            let flag_name = self.plando.game_data.flag_isv.keys[flag_id].clone();
                            let flag_details = details.flags.iter().find(
                                |x| x.flag == flag_name
                            ).unwrap();
                            details_name = flag_name;
                            details_location = flag_details.location.room.clone() + ": " + &flag_details.location.node;
                            details_area = flag_details.location.area.clone();
                            details_obtain_route = &flag_details.obtain_route;
                            details_return_route = &flag_details.return_route;
                        }
                        _ => {
                            let room_idx = self.plando.room_id_to_idx(self.plando.start_location_data.hub_location.room_id);

                            details_name = "Hub Route".to_string();
                            details_location = self.plando.game_data.room_geometry[room_idx].name.clone();
                            details_area = String::new();
                            details_obtain_route = &self.plando.start_location_data.hub_obtain_route;
                            details_return_route = &self.plando.start_location_data.hub_return_route;
                        }
                    };
                    ui.heading(details_name);
                    ui.label("LOCATION");
                    ui.label(details_location);
                    ui.label(details_area);

                    ui.separator();
                    if let SpoilerType::Item(idx) = self.spoiler_type {
                        if let Some(item_override) = self.plando.spoiler_overrides.iter().find(|x| x.item_idx == idx) {
                            ui.label("OVERRIDE DESCRIPTION");
                            ui.label(&item_override.description);
                            ui.separator();
                        }
                    }

                    ui.label("OBTAIN ROUTE");
                    for entry in details_obtain_route {
                        ui.label(entry.room.clone() + ": " + &entry.node);
                        if !entry.strat_name.is_empty() && !entry.strat_name.starts_with("Base") {
                            ui.label("Strat: ".to_string() + &entry.strat_name);
                        }
                        if !entry.relevant_flags.is_empty() {
                            let mut str = "Relevant flags: ".to_string();
                            for flag in &entry.relevant_flags {
                                str += flag;
                            }
                            ui.label(str);
                        }
                        if let Some(x) = entry.missiles_used {
                            ui.label(format!("Missiles used: {}", x));
                        }
                        if let Some(x) = entry.supers_used {
                            ui.label(format!("Supers used: {}", x));
                        }
                        if let Some(x) = entry.power_bombs_used {
                            ui.label(format!("Power Bombs used: {}", x));
                        }
                        if let Some(x) = entry.energy_used {
                            ui.label(format!("Energy used: {}", x));
                        }
                        if let Some(x) = entry.reserves_used {
                            ui.label(format!("Reserves used: {}", x));
                        }
                    }

                    ui.separator();
                    ui.label("RETURN ROUTE");
                    for entry in details_return_route {
                        ui.label(entry.room.clone() + ": " + &entry.node);
                        if !entry.strat_name.is_empty() && !entry.strat_name.starts_with("Base") {
                            ui.label("Strat: ".to_string() + &entry.strat_name);
                        }
                        if !entry.relevant_flags.is_empty() {
                            let mut str = "Relevant flags: ".to_string();
                            for flag in &entry.relevant_flags {
                                str += flag;
                            }
                            ui.label(str);
                        }
                        if let Some(x) = entry.missiles_used {
                            ui.label(format!("Missiles used: {}", x));
                        }
                        if let Some(x) = entry.supers_used {
                            ui.label(format!("Supers used: {}", x));
                        }
                        if let Some(x) = entry.power_bombs_used {
                            ui.label(format!("Power Bombs used: {}", x));
                        }
                        if let Some(x) = entry.energy_used {
                            ui.label(format!("Energy used: {}", x));
                        }
                        if let Some(x) = entry.reserves_used {
                            ui.label(format!("Reserves used: {}", x));
                        }
                    }

                    self.spoiler_type = new_spoiler_type;
                });
            }).unwrap().response;
        window.contains_pointer()
    }

    fn draw_spoiler_summary(&mut self, ctx: &Context, mouse_y: f32, spoiler_window_bounds: FloatRect) -> FloatRect {
        let mut spoiler_window_bounds = spoiler_window_bounds;
        let resp_opt = egui::Window::new("Spoiler Summary")
        .resizable(false).movable(false).title_bar(false).min_width(320.0)
        .fixed_pos(Vec2::new(16.0, 32.0).to_pos2()).show(ctx, |ui| {
            let (_r, spoiler_log) = self.plando.randomization.as_ref().unwrap();
            let ui_builder = egui::UiBuilder::new().sense(Sense::click());

            let resp = ui.scope_builder(ui_builder, |ui| {
                let spoiler_step = self.spoiler_step;
                egui::Grid::new("spoiler_summary").num_columns(2).with_row_color(move |row, _style| {
                    if row != spoiler_step { Some(Color32::from_rgb(0x20, 0x20, 0x20)) } else { Some(Color32::from_rgb(0x40, 0x40, 0x74)) }
                }).show(ui, |ui| {
                    ui.style_mut().spacing.item_spacing = Vec2::new(2.0, 2.0);
                    let mut items_found = [false; ITEM_VALUES.len()];
                    for summary in &spoiler_log.summary {
                        ui.label(summary.step.to_string());
                        ui.horizontal_wrapped(|ui| {
                            for item_summary in &summary.items {
                                let item = self.plando.game_data.item_isv.index_by_key[&item_summary.item];
                                if items_found[item] {
                                    continue;
                                }
                                items_found[item] = true;
                                let placeable_id = Placeable::ETank as u64 + item as u64;
                                let img = egui::Image::new(self.user_tex_source.get_image_source(placeable_id))
                                    .fit_to_exact_size(Vec2::new(16.0, 16.0) * self.settings.ui_scale).sense(Sense::click());
                                if ui.add(img).clicked() {
                                    self.spoiler_step = summary.step - 1;
                                    let item_loc = self.plando.game_data.item_locations.iter().position(
                                        |x| x.0 == item_summary.location.room_id && x.1 == item_summary.location.node_id
                                    ).unwrap();
                                    self.spoiler_type = SpoilerType::Item(item_loc);
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            }).response;
            
            let local_my = mouse_y - spoiler_window_bounds.top;
            let idx_unchecked = (spoiler_log.summary.len() as f32 * local_my / spoiler_window_bounds.height).floor() as i32;
            let idx = min(max(idx_unchecked, 0), spoiler_log.summary.len() as i32 - 1) as usize;
            if resp.clicked() {
                self.spoiler_step = idx;
            }
            if resp.double_clicked() && !spoiler_log.summary[idx].items.is_empty() {
                self.spoiler_step = idx;
                let item_loc = &spoiler_log.summary[idx].items[0].location;
                let item = self.plando.game_data.item_locations.iter().position(
                    |x| x.0 == item_loc.room_id && x.1 == item_loc.node_id
                ).unwrap();
                self.spoiler_type = SpoilerType::Item(item);
            }
        }).map(|x| x.response);
        if let Some(resp) = resp_opt {
            spoiler_window_bounds = FloatRect::new(resp.rect.left(), resp.rect.top(), resp.rect.width(), resp.rect.height());
        }

        spoiler_window_bounds
    }

    fn draw_settings_window(&mut self, ctx: &Context) -> bool {
        let mut settings_open = true;
        let settings_path_str = self.settings_path.clone();
        let settings_path = Path::new(&settings_path_str);
        
        egui::Window::new("Settings")
        .resizable(false)
        .title_bar(false)
        .show(ctx, |ui| {
            egui::Grid::new("grid_settings").num_columns(3).striped(true).show(ui, |ui| {
                let default = Settings::default();
                
                ui.label("Click delay tolerance").on_hover_text("Time in frames between a click and release to count as a click and not a drag");
                ui.add(egui::DragValue::new(&mut self.settings.mouse_click_delay_tolerance).range(1..=600));
                if ui.add_enabled(self.settings.mouse_click_delay_tolerance != default.mouse_click_delay_tolerance, egui::Button::new("Reset")).clicked() {
                    self.settings.mouse_click_delay_tolerance = default.mouse_click_delay_tolerance;
                }
                ui.end_row();

                ui.label("Click position tolerance").on_hover_text("Distance in pixels between a mouse click and release to count as a click and not a drag");
                ui.add(egui::DragValue::new(&mut self.settings.mouse_click_pos_tolerance).range(1..=600));
                if ui.add_enabled(self.settings.mouse_click_pos_tolerance != default.mouse_click_pos_tolerance, egui::Button::new("Reset")).clicked() {
                    self.settings.mouse_click_pos_tolerance = default.mouse_click_pos_tolerance;
                }
                ui.end_row();

                ui.label("ROM Path").on_hover_text("Path to your vanilla Super Metroid ROM");
                let mut rom_path_text = if self.settings.rom_path.is_empty() { "Empty".to_string() } else { self.settings.rom_path.clone() };
                let old_str_len = rom_path_text.len();
                rom_path_text.truncate(32);
                if old_str_len > 32 {
                    rom_path_text += "...";
                }
                if ui.button(rom_path_text).clicked() {
                    let file_opt = FileDialog::new().add_filter("Snes ROM", &["smc", "sfc"]).set_directory("/").pick_file();
                    if let Some(file) = file_opt {
                        self.settings.rom_path = file.to_str().unwrap().to_string();
                        match load_vanilla_rom(Path::new(&self.settings.rom_path)) {
                            Ok(rom) => self.rom_vanilla = Some(rom),
                            Err(err) => self.modal_type = ModalType::Error(err.to_string())
                        }
                    }
                }
                ui.end_row();

                ui.label("Auto update Spoiler").on_hover_text("If checked will automatically update the spoiler after each change. Does affect performance. Press F5 to manually update spoiler");
                ui.checkbox(&mut self.settings.spoiler_auto_update, "Auto update Spoiler");
                if ui.add_enabled(self.settings.spoiler_auto_update != default.spoiler_auto_update, egui::Button::new("Reset")).clicked() {
                    self.settings.spoiler_auto_update = default.spoiler_auto_update;
                }
                ui.end_row();

                ui.label("Disable logic").on_hover_text("Does not gray out unreachable locations. Useful for modelling out-of-logic");
                ui.checkbox(&mut self.settings.disable_logic, "Disable logic");
                if ui.add_enabled(self.settings.disable_logic != default.disable_logic, egui::Button::new("Reset")).clicked() {
                    self.settings.disable_logic = default.disable_logic;
                }
                ui.end_row();

                ui.label("Disable background grid").on_hover_text("Disables the grid of dotted lines making up the background");
                ui.checkbox(&mut self.settings.disable_bg_grid, "Disable background grid");
                if ui.add_enabled(self.settings.disable_bg_grid != default.disable_bg_grid, egui::Button::new("Reset")).clicked() {
                    self.settings.disable_bg_grid = default.disable_bg_grid;
                }
                ui.end_row();

                ui.label("Auto Updates").on_hover_text("Checks github for latest releases and updates to newer versions if possible");
                ui.checkbox(&mut self.settings.auto_update, "Check for update on Startup");
                if ui.add_enabled(self.settings.auto_update != default.auto_update, egui::Button::new("Reset")).clicked() {
                    self.settings.auto_update = default.auto_update;
                }
                ui.end_row();

                ui.label("UI Scale").on_hover_text("Scales the entire UI by this factor");
                let slider = egui::Slider::new(&mut self.settings.ui_scale, 0.5..=3.0);
                if ui.add(slider).drag_stopped() {
                    ctx.all_styles_mut(|style| {
                        let mut def = default_text_styles();
                        for (_, font_id) in &mut def {
                            font_id.size *= self.settings.ui_scale;
                        }
                        style.text_styles = def;
                    });
                }
                if ui.add_enabled(self.settings.ui_scale != default.ui_scale, egui::Button::new("Reset")).clicked() {
                    self.settings.ui_scale = default.ui_scale;
                    ctx.all_styles_mut(|style| style.text_styles = default_text_styles());
                }
                ui.end_row();

                ui.label("Scroll Speed").on_hover_text("Distance in pixels scrolled when scrolling using the Mouse Wheel");
                let slider = egui::Slider::new(&mut self.settings.scroll_speed, 1.0..=300.0);
                ui.add(slider);
                ui.end_row();

                ui.label("FPS Limit").on_hover_text("Maximal FPS Limit. Lower numbers result in lower resource consumption");
                let slider = egui::Slider::new(&mut self.settings.fps_cap, 15..=1000);
                ui.add(slider);
                ui.end_row();

                ui.label("Animation Speed").on_hover_text("Speeds up or slows down animations, such as the selection box. This has no performance impact");
                let slider = egui::Slider::new(&mut self.settings.animation_speed, 0.01..=10.0);
                ui.add(slider);
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.update_settings();
                    if let Err(err) = save_settings(&self.settings, settings_path) {
                        self.modal_type = ModalType::Error(err.to_string());
                    }
                    settings_open = false;
                }
                if ui.button("Reset All").clicked() {
                    self.settings = Settings::default();
                }
            });
        });

        settings_open
    }

}

fn main() {
    let work_dir = Path::new("./data/maprando-data/");
    std::env::set_current_dir(work_dir).unwrap();

    let mut plando_app = PlandoApp::new().unwrap();
    plando_app.render_loop();
}



struct ImplUserTexSource {
    tex_map: HashMap<u64, FBox<graphics::Texture>>
}

impl ImplUserTexSource {
    fn new() -> Self {
        ImplUserTexSource { tex_map: HashMap::new() }
    }

    fn add_texture(&mut self, id: u64, tex: FBox<graphics::Texture>) {
        self.tex_map.insert(id, tex);
    }

    fn get_image_source(&mut self, tex_id: u64) -> (TextureId, egui::Vec2) {
        let (x, y, _tex) = self.get_texture(tex_id);
        (TextureId::User(tex_id), egui::Vec2::new(x, y))
    }
}

impl UserTexSource for ImplUserTexSource {
    fn get_texture(&mut self, id: u64) -> (f32, f32, &graphics::Texture) {
        let tex = anyhow::Context::context(self.tex_map.get(&id), "Invalid texture id provided").unwrap();
        (tex.size().x as f32, tex.size().y as f32, tex)
    }
}