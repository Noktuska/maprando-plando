use {
    anyhow::{anyhow, bail, Context, Result}, egui_sfml::{egui::{self, Color32, Id, Sense, TextureId, Vec2}, SfEgui, UserTexSource}, flate2::read::GzDecoder, hashbrown::HashMap, maprando::{
        customize::{mosaic::MosaicTheme, ControllerButton, ControllerConfig, CustomizeSettings, DoorTheme, FlashingSetting, MusicSettings, PaletteTheme, ShakingSetting, TileTheme}, patch::Rom, randomize::SpoilerRouteEntry, settings::{DoorLocksSize, DoorsMode, ItemDotChange, MapStationReveal, MapsRevealed, Objective, ObjectiveSetting, RandomizerSettings, SaveAnimals, WallJump}
    }, maprando_game::{BeamType, DoorType, GameData, Item, Map, MapTileEdge, MapTileInterior, MapTileSpecialType}, plando::{DoubleItemPlacement, MapRepositoryType, Placeable, Plando, ITEM_VALUES}, rand::RngCore, rfd::FileDialog, self_update::cargo_crate_version, serde::{Deserialize, Serialize}, sfml::{
        cpp::FBox, graphics::{
            self, Color, FloatRect, IntRect, PrimitiveType, RenderStates, RenderTarget, RenderWindow, Shape, Transformable, Vertex
        }, system::{Vector2f, Vector2i}, window::{
            mouse, Event, Key, Style
        }
    }, std::{cmp::{max, min}, fs::File, io::{Read, Write}, path::Path, thread::{self, JoinHandle}, u32}
};

mod plando;
mod layout;

#[derive(Clone)]
struct RoomData {
    room_id: usize,
    room_idx: usize,
    room_name: String,
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
    customization: Customization,
    last_logic_preset: Option<RandomizerSettings>,
    disable_logic: bool,
    auto_update: bool,
    disable_bg_grid: bool
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            mouse_click_pos_tolerance: 5,
            mouse_click_delay_tolerance: 60,
            rom_path: String::new(),
            spoiler_auto_update: true,
            customization: Customization::default(),
            last_logic_preset: None,
            disable_logic: false,
            auto_update: true,
            disable_bg_grid: false,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum CustomControllerButton {
    Left,
    Right,
    Up,
    Down,
    X,
    Y,
    A,
    B,
    L,
    R,
    Select,
    Start,
}

impl CustomControllerButton {
    fn convert(&self) -> ControllerButton {
        use ControllerButton::*;
        match self {
            CustomControllerButton::Left => Left,
            CustomControllerButton::Right => Right,
            CustomControllerButton::Up => Up,
            CustomControllerButton::Down => Down,
            CustomControllerButton::X => X,
            CustomControllerButton::Y => Y,
            CustomControllerButton::A => A,
            CustomControllerButton::B => B,
            CustomControllerButton::L => L,
            CustomControllerButton::R => R,
            CustomControllerButton::Select => Select,
            CustomControllerButton::Start => Start,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CustomControllerConfig {
    pub shot: CustomControllerButton,
    pub jump: CustomControllerButton,
    pub dash: CustomControllerButton,
    pub item_select: CustomControllerButton,
    pub item_cancel: CustomControllerButton,
    pub angle_up: CustomControllerButton,
    pub angle_down: CustomControllerButton,
    pub spin_lock_buttons: Vec<CustomControllerButton>,
    pub quick_reload_buttons: Vec<CustomControllerButton>,
    pub moonwalk: bool,
}

impl CustomControllerConfig {
    fn default() -> Self {
        use CustomControllerButton::*;
        CustomControllerConfig {
            shot: X,
            jump: A,
            dash: B,
            item_select: Select,
            item_cancel: Y,
            angle_up: R,
            angle_down: L,
            spin_lock_buttons: vec![X, L, R, Up],
            quick_reload_buttons: vec![L, R, Select, Start],
            moonwalk: false
        }
    }

    fn is_valid(&self) -> bool {
        let mut vec = vec![];
        vec.push(self.shot as usize);
        vec.push(self.jump as usize);
        vec.push(self.dash as usize);
        vec.push(self.item_cancel as usize);
        vec.push(self.item_select as usize);
        vec.push(self.angle_down as usize);
        vec.push(self.angle_up as usize);
        vec.sort();
        vec.dedup();
        vec.len() == 7
    }

    fn to_controller_config(&self) -> ControllerConfig {
        ControllerConfig {
            shot: self.shot.convert(),
            jump: self.jump.convert(),
            dash: self.dash.convert(),
            item_select: self.item_select.convert(),
            item_cancel: self.item_cancel.convert(),
            angle_up: self.angle_up.convert(),
            angle_down: self.angle_down.convert(),
            spin_lock_buttons: self.spin_lock_buttons.iter().map(|x| x.convert()).collect(),
            quick_reload_buttons: self.quick_reload_buttons.iter().map(|x| x.convert()).collect(),
            moonwalk: self.moonwalk
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Customization {
    pub samus_sprite: String,
    pub etank_color: [f32; 3],
    pub reserve_hud_style: bool,
    pub vanilla_screw_attack_animation: bool,
    pub palette_theme: usize,
    pub tile_theme: usize,
    pub door_theme: usize,
    pub music: usize,
    pub disable_beeping: bool,
    pub shaking: usize,
    pub flashing: usize,
    pub controller_config: CustomControllerConfig,
}

impl Customization {
    fn default() -> Self {
        Customization {
            samus_sprite: "samus_vanilla".to_string(),
            etank_color: [0xDE as f32 / 255.0, 0x38 as f32 / 255.0, 0x94 as f32 / 255.0],
            reserve_hud_style: true,
            vanilla_screw_attack_animation: false,
            palette_theme: 0,
            tile_theme: 0,
            door_theme: 0,
            music: 0,
            disable_beeping: false,
            shaking: 1,
            flashing: 1,
            controller_config: CustomControllerConfig::default()
        }
    }
    
    fn to_settings(&self, themes: &[MosaicTheme]) -> CustomizeSettings {
        let etank_color = Some((
            (self.etank_color[0] * 31.0) as u8,
            (self.etank_color[1] * 31.0) as u8,
            (self.etank_color[2] * 31.0) as u8
        ));

        let palette_theme = match self.palette_theme {
            1 => PaletteTheme::AreaThemed,
            _ => PaletteTheme::Vanilla
        };
        let tile_theme = match self.tile_theme {
            0 => TileTheme::Vanilla,
            1 => TileTheme::AreaThemed,
            2 => TileTheme::Scrambled,
            i => {
                let idx = i - 2;
                if idx == themes.len() {
                    TileTheme::Constant("Outline".to_string())
                } else if idx > themes.len() {
                    TileTheme::Constant("Invisible".to_string())
                } else {
                    TileTheme::Constant(themes[idx].name.clone())
                }
            }
        };
        let door_theme = match self.door_theme {
            1 => DoorTheme::Alternate,
            _ => DoorTheme::Vanilla
        };
        let music = match self.music {
            1 => MusicSettings::Disabled,
            _ => MusicSettings::AreaThemed
        };
        let shaking = match self.shaking {
            1 => ShakingSetting::Reduced,
            2 => ShakingSetting::Disabled,
            _ => ShakingSetting::Vanilla
        };
        let flashing = match self.flashing {
            1 => FlashingSetting::Reduced,
            _ => FlashingSetting::Vanilla
        };

        CustomizeSettings {
            samus_sprite: Some(self.samus_sprite.clone()),
            etank_color,
            reserve_hud_style: self.reserve_hud_style,
            vanilla_screw_attack_animation: self.vanilla_screw_attack_animation,
            palette_theme,
            tile_theme,
            door_theme,
            music,
            disable_beeping: self.disable_beeping,
            shaking,
            flashing,
            controller_config: self.controller_config.to_controller_config()
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
struct StartLocationSerializable {
    name: String,
    room_id: usize,
    node_id: usize,
    door_load_id: Option<usize>,
    x: f32,
    y: f32,
    requires: Option<Vec<serde_json::Value>>,
    note: Option<Vec<String>>,
    camera_offset_x: Option<f32>,
    camera_offset_y: Option<f32>,
}

#[derive(Serialize, Deserialize)]
struct LockedDoorSerializable {
    room_id: usize,
    node_id: usize,
    door_type: usize,
    direction: String,
}

#[derive(Serialize, Deserialize)]
struct SeedData {
    map: Map,
    start_location: usize,
    item_placements: Vec<Item>,
    door_locks: Vec<LockedDoorSerializable>,
    settings: RandomizerSettings
}

fn save_settings(settings: &Settings, path: &Path) -> Result<()> {
    let mut file = File::create(path)?;
    let data = serde_json::to_string_pretty(&settings)?;
    file.write_all(data.as_bytes())?;
    Ok(())
}

fn load_settings(path: &Path) -> Result<Settings> {
    let mut file = File::open(path)?;
    let mut data_str = String::new();
    file.read_to_string(&mut data_str)?;
    let result: Settings = serde_json::from_str(&data_str)?;
    Ok(result)
}

fn get_seed_data(plando: &Plando) -> SeedData {
    let mut door_locks = Vec::new();
    for door_lock in &plando.locked_doors {
        let (room_id, node_id) = plando.game_data.door_ptr_pair_map[&door_lock.src_ptr_pair];
        let (room_idx, door_idx) = plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door_lock.src_ptr_pair];
        let direction = plando.game_data.room_geometry[room_idx].doors[door_idx].direction.clone();

        let door_type = match door_lock.door_type {
            DoorType::Beam(beam) => 5 + beam as usize,
            DoorType::Blue => 0,
            DoorType::Gray => 1,
            DoorType::Red => 2,
            DoorType::Green => 3,
            DoorType::Yellow => 4
        };

        door_locks.push(LockedDoorSerializable {
            room_id,
            node_id,
            door_type,
            direction
        });
    }

    let start_room_id = plando.start_location_data.start_location.room_id;
    let start_node_id = plando.start_location_data.start_location.node_id;
    let start_location_id = if start_room_id == 8 && start_node_id == 5 {
        plando.game_data.start_locations.len()
    } else {
        plando.game_data.start_location_id_map[&(start_room_id, start_node_id)]
    };

    SeedData {
        map: plando.map.clone(),
        start_location: start_location_id,
        item_placements: plando.item_locations.clone(),
        door_locks,
        settings: plando.randomizer_settings.clone()
    }
}

fn load_seed(plando: &mut Plando, path: &Path) -> Result<()> {
    let mut file = File::open(path)?;

    let mut str = String::new();
    file.read_to_string(&mut str)?;

    let seed_data: SeedData = serde_json::from_str(&str)?;

    let auto_update = plando.auto_update_spoiler;
    plando.auto_update_spoiler = false;

    plando.load_preset(seed_data.settings);

    plando.clear_item_locations();
    plando.clear_doors();

    plando.load_map(seed_data.map);
    let start_location = if seed_data.start_location == plando.game_data.start_locations.len() {
        Plando::get_ship_start()
    } else {
        plando.game_data.start_locations[seed_data.start_location].clone()
    };

    plando.item_locations = seed_data.item_placements;
    for item in &plando.item_locations {
        if *item != Item::Nothing {
            plando.placed_item_count[*item as usize + Placeable::ETank as usize] += 1;
        }
    }
    
    for door_data in seed_data.door_locks {
        let door_type = match door_data.door_type {
            1 => DoorType::Gray,
            2 => DoorType::Red,
            3 => DoorType::Green,
            4 => DoorType::Yellow,
            5 => DoorType::Beam(BeamType::Charge),
            6 => DoorType::Beam(BeamType::Ice),
            7 => DoorType::Beam(BeamType::Wave),
            8 => DoorType::Beam(BeamType::Spazer),
            9 => DoorType::Beam(BeamType::Plasma),
            _ => DoorType::Blue
        };
        
        let (tile_x, tile_y) = plando.game_data.node_coords[&(door_data.room_id, door_data.node_id)];
        let room_idx = plando.room_id_to_idx(door_data.room_id);
        
        let door_idx = plando.get_door_idx(room_idx, tile_x, tile_y, door_data.direction).ok_or_else(|| anyhow!("Malformed Door Data"))?;
        plando.place_door(room_idx, door_idx, Some(door_type), false, true)?;
    }

    plando.place_start_location(start_location)?;

    plando.update_spoiler_data();
    plando.auto_update_spoiler = auto_update;

    Ok(())
}

fn save_seed(plando: &Plando, path: &Path) -> Result<()> {
    let mut file = File::create(path)?;

    let seed_data = get_seed_data(plando);

    let out = serde_json::to_string(&seed_data)?;

    file.write_all(out.as_bytes())?;
    Ok(())
}

fn load_map(plando: &mut Plando, path: &Path) -> Result<()> {
    let mut file = File::open(path)?;
    let mut data_str = String::new();
    file.read_to_string(&mut data_str)?;
    let map: Map = serde_json::from_str(&data_str)?;
    plando.load_map(map);
    Ok(())
}

fn save_preset(preset: &RandomizerSettings) -> Result<()> {
    let str = serde_json::to_string_pretty(preset)?;
    let dir = Path::new("./data/presets/full-settings/");
    let path = dir.join(preset.name.as_ref().unwrap());
    let mut file = File::create(path)?;
    file.write_all(str.as_bytes())?;

    Ok(())
}

fn download_map_repos() -> Result<()> {
    for pool in ["v117c-standard", "v117c-wild"] {
        let url = format!("https://map-rando-artifacts.s3.us-west-004.backblazeb2.com/maps/{pool}.tgz");
        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send()?;

        println!("Attempting to download {url}");
        let bytes = resp.bytes()?;
        let cursor = std::io::Cursor::new(&bytes);
        let decoder = GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(decoder);
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

    match file_ext {
        "exe" => {}
        "zip" => bail!("Extracting ZIP files is not yet supported, please manually download and install update"),
        _ => bail!("Unexpected file type")
    }

    std::fs::create_dir_all(tmp_dir_path)?;
    let tmp_file = File::create(&tmp_archive_path)?;

    println!("Downloading...");
    self_update::Download::from_url(&asset.download_url)
        .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
        .download_to(tmp_file)?;

    println!("Replacing executable...");
    self_update::self_replace::self_replace(&tmp_archive_path)?;

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
            room_name: map_tile_data.room_name.clone(),
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
    let mut img_doors = graphics::Image::new_solid(3 * 9, 8, Color::TRANSPARENT).unwrap();
    for x in 0..9 {
        let door_color_index = match x {
            0 | 5 => 7,
            1 | 7 => 14,
            2 | 6 => 6,
            3 | 8 => 15,
            4 => 8,
            _ => 15
        };
        
        img_doors.set_pixel(3 * x + 2, 3, get_explored_color(12, 0))?;
        img_doors.set_pixel(3 * x + 2, 4, get_explored_color(12, 0))?;
        
        img_doors.set_pixel(3 * x, 3, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x + 1, 3, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x, 4, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x + 1, 4, get_explored_color(door_color_index, 0))?;
        
        if x < 3 || x == 8 {
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

fn patch_rom(plando: &Plando, rom_vanilla: &Rom, settings: &CustomizeSettings, save_path: &Path) -> Result<()> {
    if plando.randomization.is_none() {
        bail!("No randomization generated");
    }
    let (r, _spoiler_log) = plando.randomization.as_ref().unwrap();
    let mut new_rom = maprando::patch::make_rom(&rom_vanilla, &plando.randomizer_settings, r, &plando.game_data)?;
    let map = plando.map.clone();

    maprando::customize::customize_rom(
        &mut new_rom,
        rom_vanilla,
        &Some(map),
        settings,
        &plando.game_data,
        &plando.samus_sprite_categories,
        &plando.mosaic_themes
    )?;

    let mut file = File::create(save_path)?;
    file.write_all(&new_rom.data)?;

    Ok(())
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
    Bosses = Placeable::VALUES.len() as isize + 1,
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

fn main() {
    let work_dir = Path::new("./data/maprando-data/");
    std::env::set_current_dir(work_dir).unwrap();

    let mut plando = Plando::new();

    let settings_path = Path::new("../plando_settings.json");
    let mut settings = load_settings(settings_path).unwrap_or_default();

    if settings.auto_update {
        match check_update() {
            Ok(_) => println!("Successful!"),
            Err(err) => println!("ERR: {}", err.to_string())
        };
    }

    plando.auto_update_spoiler = settings.spoiler_auto_update;
    if let Some(preset) = &settings.last_logic_preset {
        plando.load_preset(preset.clone());
    }

    let mut rom_vanilla = load_vanilla_rom(&Path::new(&settings.rom_path)).ok();

    let (atlas_img, room_data) = load_room_sprites(&plando.game_data).unwrap();
    let atlas_tex = graphics::Texture::from_image(&atlas_img, IntRect::default()).unwrap();

    let mut tex_grid = graphics::Texture::from_file("../visualizer/grid.png").unwrap();
    tex_grid.set_repeated(true);

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

    let mut window = RenderWindow::new((1080, 720), "Maprando Plando", Style::DEFAULT, &Default::default()).expect("Could not create Window");
    window.set_vertical_sync_enabled(true);

    let font_default = graphics::Font::from_file("../segoeui.ttf").expect("Could not load default font");

    let mut x_offset = 0.0;
    let mut y_offset = 0.0;
    let mut zoom = 1.0;

    let mut is_mouse_down = false;
    let mut is_mouse_clicked: Option<mouse::Button>;
    let mut mouse_click_pos = Vector2i::new(0, 0);
    let mut mouse_click_timer = 0;
    let mut mouse_x = 0;
    let mut mouse_y = 0;

    let img_items = graphics::Image::from_file("../visualizer/items.png").unwrap();
    let tex_items = graphics::Texture::from_image(&img_items, IntRect::default()).unwrap();
    let tex_item_width = (tex_items.size().x / 24) as i32;

    let img_doors = generate_door_sprites().unwrap();
    let img_door_width = (img_doors.size().x / 8) as i32;

    let tex_helm = graphics::Texture::from_file("../visualizer/helm.png").unwrap();
    let tex_bosses = graphics::Texture::from_file("../visualizer/bosses.png").unwrap();
    let tex_minibosses = graphics::Texture::from_file("../visualizer/minibosses.png").unwrap();
    let tex_misc = graphics::Texture::from_file("../visualizer/misc.png").unwrap();
    let mut user_tex_source = ImplUserTexSource::new();
    
    user_tex_source.add_texture(Placeable::Helm as u64, tex_helm);
    user_tex_source.add_texture(FlagType::Bosses as u64, tex_bosses);
    user_tex_source.add_texture(FlagType::Minibosses as u64, tex_minibosses);
    user_tex_source.add_texture(FlagType::Misc as u64, tex_misc);

    // Add item textures to egui
    for i in 0..22 {
        let source_rect = IntRect::new(i * tex_item_width, 0, tex_item_width, img_items.size().y as i32);
        let tex = graphics::Texture::from_image(&img_items, source_rect).unwrap();
        user_tex_source.add_texture(Placeable::ETank as u64 + i as u64, tex);
    }
    // Add Door textures to egui
    for i in 0..9 {
        let source_rect = IntRect::new(i * img_door_width, 0, img_door_width, img_doors.size().y as i32);
        let tex = graphics::Texture::from_image(&img_doors, source_rect).unwrap();
        user_tex_source.add_texture(Placeable::DoorMissile as u64 + i as u64, tex);
    }
    const TEX_DOOR_GRAY: u64 = 31;
    // Add Flag textures to egui
    const FLAG_TEX_START: u64 = FlagType::Misc as u64 + 1;
    let mut flag_has_tex = Vec::new();
    for flag_idx in 0..plando.game_data.flag_ids.len() {
        let flag_id = plando.game_data.flag_ids[flag_idx];
        let flag_str = &plando.game_data.flag_isv.keys[flag_id];
        if let Ok(tex) = graphics::Texture::from_file(("../visualizer/".to_string().to_owned() + flag_str + ".png").as_str()) {
            user_tex_source.add_texture(FLAG_TEX_START + flag_id as u64, tex);
            flag_has_tex.push(flag_id);
        }
    }

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

    let mut cur_settings = plando.randomizer_settings.clone();
    enum CustomizeLogicWindow {
        None, SkillAssumption, ItemProgression, Qol, Objectives
    }
    let mut cur_customize_logic_window = CustomizeLogicWindow::None;
    let mut is_customize_window_open = true;

    let mut sidebar_width = 0.0;
    let sidebar_height = 32.0;

    let mut sfegui = SfEgui::new(&window);

    let mut error_modal_message: Option<String> = None;
    let mut settings_open = false;
    let mut customize_open = false;
    //let mut character_select_open = false;
    let mut customize_logic_open = false;
    let mut reset_after_patch = false;

    let mut sidebar_selection: Option<Placeable> = None;
    let mut spoiler_step = 0;
    let mut spoiler_type = SpoilerType::None;
    let mut spoiler_window_bounds = FloatRect::default();
    let mut spoiler_details_hovered = false;

    let mut download_thread_active = false;
    let mut download_thread_handle: JoinHandle<Result<(), anyhow::Error>> = thread::spawn(|| { Ok(()) });
    let mut status_message = None;

    while window.is_open() {
        let local_mouse_x = (mouse_x as f32 - x_offset) / zoom;
        let local_mouse_y = (mouse_y as f32 - y_offset) / zoom;
        let tile_x = (local_mouse_x / 8.0).floor().max(0.0) as usize;
        let tile_y = (local_mouse_y / 8.0).floor().max(0.0) as usize;
        let tile_hovered_opt = plando.get_tile_at(tile_x, tile_y);

        is_mouse_clicked = None;
        if mouse_click_timer > 0 {
            mouse_click_timer -= 1;
        }
        let mut click_consumed = false;

        if download_thread_active {
            if download_thread_handle.is_finished() {
                let res = download_thread_handle.join().unwrap();
                match res {
                    Ok(_) => {},
                    Err(err) => error_modal_message = Some(err.to_string())
                }
                status_message = None;
                download_thread_active = false;
                download_thread_handle = thread::spawn(|| { Ok(()) }); // Reset the handle
            }
        }

        let ignore_click = spoiler_details_hovered || settings_open || customize_open || status_message.is_some() || error_modal_message.is_some() || customize_logic_open;

        while let Some(ev) = window.poll_event() {
            sfegui.add_event(&ev);

            match ev {
                Event::Closed => {
                    let _ = save_settings(&settings, settings_path);
                    window.close();
                }
                Event::MouseButtonPressed { button, x, y } => {
                    if x < window.size().x as i32 - sidebar_width as i32 && !ignore_click && !spoiler_window_bounds.contains2(x as f32, y as f32) {
                        if button == mouse::Button::Left {
                            is_mouse_down = true;
                        } else if button == mouse::Button::Middle {
                            x_offset = 0.0;
                            y_offset = 0.0;
                            zoom = 1.0;
                        }
                    }

                    mouse_click_pos.x = x;
                    mouse_click_pos.y = y;
                    mouse_click_timer = settings.mouse_click_delay_tolerance;
                },
                Event::MouseButtonReleased { button, x, y } => {
                    if button == mouse::Button::Left {
                        is_mouse_down = false;
                    }

                    let new_mouse_pos = Vector2i::new(x, y);
                    if (mouse_click_pos - new_mouse_pos).length_sq() < settings.mouse_click_pos_tolerance && mouse_click_timer > 0 && !ignore_click && !spoiler_window_bounds.contains2(x as f32, y as f32) {
                        is_mouse_clicked = Some(button);
                    }
                },
                Event::MouseWheelScrolled { wheel: _, delta, x, y } => {
                    if x < window.size().x as i32 - sidebar_width as i32 && !ignore_click && !spoiler_window_bounds.contains2(x as f32, y as f32) {
                        let factor = 1.1;
                        if delta > 0.0 && zoom < 20.0 {
                            zoom *= factor;
                            x_offset -= (factor - 1.0) * (mouse_x as f32 - x_offset);
                            y_offset -= (factor - 1.0) * (mouse_y as f32 - y_offset);
                        } else if delta < 0.0 && zoom > 0.1 {
                            zoom /= factor;
                            x_offset += (1.0 - 1.0 / factor) * (mouse_x as f32 - x_offset);
                            y_offset += (1.0 - 1.0 / factor) * (mouse_y as f32 - y_offset);
                        }
                    }
                },
                Event::MouseMoved { x, y } => {
                    let dx = x - mouse_x;
                    let dy = y - mouse_y;
                    if is_mouse_down {
                        x_offset += dx as f32;
                        y_offset += dy as f32;
                    }
                    mouse_x = x;
                    mouse_y = y;
                },
                Event::Resized { width, height } => {
                    window.set_view(&graphics::View::from_rect(graphics::Rect::new(0.0, 0.0, width as f32, height as f32)).unwrap());
                },
                Event::KeyPressed { code, .. } => {
                    if code == Key::F5 {
                        plando.update_spoiler_data();
                    } else if code == Key::Add {
                        spoiler_step += 1;
                    } else if code == Key::Subtract && spoiler_step > 0 {
                        spoiler_step -= 1;
                    }
                }
                _ => {}
            }
        }

        spoiler_details_hovered = false;

        window.clear(Color::rgb(0x1F, 0x1F, 0x1F));

        let mut states = graphics::RenderStates::default();
        states.transform.translate(x_offset, y_offset);
        states.transform.scale(zoom, zoom);

        let mut info_overlay_opt: Option<String> = None;
        // Don't render map if we're patching from a seed file to not spoiler the user
        if !reset_after_patch {
            // Draw background grid
            if !settings.disable_bg_grid {
                let spr_bg_grid = graphics::Sprite::with_texture_and_rect(&tex_grid, IntRect::new(0, 0, 8 * 75, 8 * 75));
                window.draw_with_renderstates(&spr_bg_grid, &states);
            }

            // Draw the entire map
            for i in 0..room_data.len() {
                let data = &room_data[i];
                let (x, y) = plando.map.rooms[data.room_idx];
                let room_geometry = &plando.game_data.room_geometry[data.room_idx];

                let is_objective = data.room_id == 238 || obj_room_map.get(&data.room_id).is_some_and(
                    |obj| plando.objectives.contains(obj)
                );

                // Draw the background color
                for (local_y, row) in room_geometry.map.iter().enumerate() {
                    for (local_x, &cell) in row.iter().enumerate() {
                        // Ignore map tiles on toilet
                        if cell == 0 && data.room_id != 321 {
                            continue;
                        }

                        let mut color_div = 1;
                        if !settings.disable_logic {
                            if let Some((_r, spoiler_log)) = &plando.randomization {
                                if spoiler_log.all_rooms[data.room_idx].map_bireachable_step[local_y][local_x] > spoiler_step as u8 {
                                    color_div *= 2;
                                }
                                if spoiler_log.all_rooms[data.room_idx].map_reachable_step[local_y][local_x] > spoiler_step as u8 {
                                    color_div *= 3;
                                }
                            }
                        }

                        let cell_x = (local_x + x) * 8;
                        let cell_y = (local_y + y) * 8;
                        let color_value = if room_geometry.heated { 2 } else { 1 };
                        let mut cell_color = get_explored_color(color_value, plando.map.area[data.room_idx]);
                        cell_color.r /= color_div;
                        cell_color.g /= color_div;
                        cell_color.b /= color_div;

                        let mut bg_rect = graphics::RectangleShape::with_size(Vector2f::new(8.0, 8.0));
                        bg_rect.set_position(Vector2f::new(cell_x as f32, cell_y as f32));
                        bg_rect.set_fill_color(cell_color);
                        window.draw_with_renderstates(&bg_rect, &states);

                        // Set up an info overlay we'll draw later, so it'll be on top
                        if info_overlay_opt.is_none() && graphics::FloatRect::new(cell_x as f32, cell_y as f32, 8.0, 8.0).contains2(local_mouse_x, local_mouse_y) {
                            let mut info_str = data.room_name.to_string();
                            if plando.start_location_data.hub_location.room_id == data.room_id {
                                info_str += " (Hub)";
                            }
                            info_overlay_opt = Some(info_str);
                        }

                        // Draw Tile Outline
                        let sprite_tile_rect = IntRect::new(8 * (data.atlas_x_offset as i32 + local_x as i32), 8 * (data.atlas_y_offset as i32 + local_y as i32), 8, 8);
                        let mut sprite_tile = graphics::Sprite::with_texture_and_rect(&atlas_tex, sprite_tile_rect);
                        sprite_tile.set_position(Vector2f::new(cell_x as f32, cell_y as f32));
                        sprite_tile.set_color(Color::rgb(255 / color_div, 255 / color_div, 255 / color_div));
                        window.draw_with_renderstates(&sprite_tile, &states);

                        if is_objective && get_objective_mask(data.room_id, local_x, local_y) {
                            sprite_tile.set_texture(&tex_obj, true);
                            window.draw_with_renderstates(&sprite_tile, &states);
                        }

                        let (tex_helm_w, _, tex_helm) = user_tex_source.get_texture(Placeable::Helm as u64);
                        let mut sprite_helm = graphics::Sprite::with_texture(tex_helm);
                        sprite_helm.set_scale(8.0 / tex_helm_w);
                    }
                }
            }

            // Draw Possible Start Locations
            {
                let (tex_helm_w, _, tex_helm) = user_tex_source.get_texture(Placeable::Helm as u64);
                let mut sprite_helm = graphics::Sprite::with_texture(tex_helm);
                sprite_helm.set_color(Color::rgba(0xAF, 0xAF, 0xAF, 0x5F));

                if sidebar_selection.is_some_and(|sel| sel == Placeable::Helm) {
                    for i in 0..plando.game_data.start_locations.len() {
                        let room_idx = plando.room_id_to_idx(plando.game_data.start_locations[i].room_id);
                        let (room_x, room_y) = plando.map.rooms[room_idx];
                        let tile_x = (plando.game_data.start_locations[i].x / 16.0).floor();
                        let tile_y = (plando.game_data.start_locations[i].y / 16.0).floor();

                        sprite_helm.set_position(Vector2f::new(room_x as f32 + tile_x, room_y as f32 + tile_y) * 8.0);
                        sprite_helm.set_scale(8.0 / tex_helm_w);

                        if sprite_helm.global_bounds().contains2(local_mouse_x, local_mouse_y) {
                            sprite_helm.scale(1.2);
                            if let Some(bt) = is_mouse_clicked {
                                let mut res = Ok(());
                                if bt == mouse::Button::Left {
                                    res = plando.place_start_location(plando.game_data.start_locations[i].clone());
                                } else if bt == mouse::Button::Right {
                                    res = plando.place_start_location(Plando::get_ship_start());
                                }
                                click_consumed = true;
                                if let Err(err) = res {
                                    error_modal_message = Some(err.to_string());
                                }
                            }
                        }

                        window.draw_with_renderstates(&sprite_helm, &states);
                    }
                }
                sprite_helm.set_color(Color::WHITE);
                sprite_helm.set_scale(8.0 / tex_helm_w);

                // Draw current start location
                let room_idx = plando.room_id_to_idx(plando.start_location_data.start_location.room_id);
                let (room_x, room_y) = plando.map.rooms[room_idx];
                let start_tile_x = (plando.start_location_data.start_location.x / 16.0).floor();
                let start_tile_y = (plando.start_location_data.start_location.y / 16.0).floor();
                sprite_helm.set_position(Vector2f::new(room_x as f32 + start_tile_x, room_y as f32 + start_tile_y) * 8.0);
                window.draw_with_renderstates(&sprite_helm, &states);

                if sidebar_selection.is_none() && sprite_helm.global_bounds().contains2(local_mouse_x, local_mouse_y) {
                    sprite_helm.scale(1.2);
                    if is_mouse_clicked.is_some_and(|x| x == mouse::Button::Left) {
                        spoiler_type = SpoilerType::Hub;
                        click_consumed = true;
                    }
                }
            }

            // Draw Doors
            for door in &plando.locked_doors {
                if door.door_type == DoorType::Blue {
                    continue;
                }

                let (room_src_idx, _door_src_idx) = plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door.src_ptr_pair];
                let (room_dst_idx, _door_dst_idx) = plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door.dst_ptr_pair];
                let room_idxs = vec![(room_src_idx, door.src_ptr_pair), (room_dst_idx, door.dst_ptr_pair)];
                for (room_idx, ptr_pair) in room_idxs {
                    let (room_x, room_y) = plando.map.rooms[room_idx];
                    let room_geometry = &plando.game_data.room_geometry[room_idx];
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
                        _ => Placeable::DoorMissile
                    } as u64;
                    let (_tex_w, _tex_h, door_tex) = user_tex_source.get_texture(door_tex_id);
                    let mut door_spr = graphics::Sprite::with_texture(door_tex);
                    door_spr.set_origin((4.0, 4.0));
                    door_spr.set_position((x + 4.0, y + 4.0));
                    door_spr.set_rotation(match dir.as_str() {
                        "up" => 90.0,
                        "right" => 180.0,
                        "down" => 270.0,
                        _ => 0.0
                    });

                    window.draw_with_renderstates(&door_spr, &states);
                }
            }

            // Draw Gray Doors
            for door_ptr_pair in &plando.gray_doors {
                let (room_idx, door_idx) = plando.game_data.room_and_door_idxs_by_door_ptr_pair[door_ptr_pair];
                let (room_x, room_y) = plando.map.rooms[room_idx];
                let room_geomtry = &plando.game_data.room_geometry[room_idx];
                let door = &room_geomtry.doors[door_idx];
                let x = (room_x + door.x) as f32 * 8.0;
                let y = (room_y + door.y) as f32 * 8.0;

                let tex = user_tex_source.get_texture(TEX_DOOR_GRAY).2;
                let mut door_spr = graphics::Sprite::with_texture(tex);
                door_spr.set_origin((4.0, 4.0));
                door_spr.set_position((x + 4.0, y + 4.0));
                door_spr.set_rotation(match door.direction.as_str() {
                    "up" => 90.0,
                    "right" => 180.0,
                    "down" => 270.0,
                    _ => 0.0
                });

                window.draw_with_renderstates(&door_spr, &states);
            }

            // Draw items
            if sidebar_selection.is_none() || sidebar_selection.is_some_and(|x| x >= Placeable::ETank) {
                for i in 0..plando.item_locations.len() {
                    let item = plando.item_locations[i];
                    let (room_id, node_id) = plando.game_data.item_locations[i];
                    let room_ptr = plando.game_data.room_ptr_by_id[&room_id];
                    let room_idx = plando.game_data.room_idx_by_ptr[&room_ptr];
                    let (room_x, room_y) = plando.map.rooms[room_idx];
                    let (tile_x, tile_y) = plando.game_data.node_coords[&(room_id, node_id)];

                    let item_index = match item {
                        Item::Nothing => 23,
                        item => item as i32
                    };

                    let mut spr_item = graphics::Sprite::with_texture_and_rect(&tex_items,
                        IntRect::new(tex_item_width * item_index, 0, tex_item_width, tex_item_width));
                    spr_item.set_origin(Vector2f::new(tex_item_width as f32 / 2.0, tex_item_width as f32 / 2.0));
                    let double_item = plando::get_double_item_offset(room_id, node_id);
                    let item_x_offset = match double_item {
                        DoubleItemPlacement::Left => 2,
                        DoubleItemPlacement::Middle => 4,
                        DoubleItemPlacement::Right => 6
                    };
                    spr_item.set_position(Vector2f::new((8 * (tile_x + room_x) + item_x_offset) as f32, (8 * (tile_y + room_y) + 4) as f32));
                    spr_item.set_scale(6.0 / tex_item_width as f32);
                    if spr_item.global_bounds().contains2(local_mouse_x, local_mouse_y) {
                        spr_item.scale(1.2);
                        let item_name = if item_index == 23 {
                            &"Nothing".to_string()
                        } else {
                            &plando.game_data.item_isv.keys[item_index as usize]
                        };
                        info_overlay_opt = Some(item_name.clone());

                        if let Some(bt) = is_mouse_clicked {
                            if bt == mouse::Button::Left {
                                if sidebar_selection.is_some_and(|x| x.to_item().is_some()) {
                                    let item_to_place = sidebar_selection.unwrap().to_item().unwrap();
                                    let as_placeable = Placeable::VALUES[item_to_place as usize + Placeable::ETank as usize];
                                    if plando.placed_item_count[as_placeable as usize] < plando.get_max_placeable_count(as_placeable).unwrap() {
                                        plando.place_item(i, item_to_place);
                                    }
                                } else {
                                    spoiler_type = SpoilerType::Item(i);
                                }
                            } else if bt == mouse::Button::Right {
                                plando.place_item(i, Item::Nothing);
                            }
                            click_consumed = true;
                        }
                    }

                    window.draw_with_renderstates(&spr_item, &states);
                }
            }

            // Draw flags
            for (i, &flag_id) in plando.game_data.flag_ids.iter().enumerate() {
                let vertex_info = &plando.get_vertex_info(plando.game_data.flag_vertex_ids[i][0]);
                let room_idx = plando.room_id_to_idx(vertex_info.room_id);
                let (room_x, room_y) = plando.map.rooms[room_idx];

                let flag_str_short = &plando.game_data.flag_isv.keys[flag_id];
                let flag_info = get_flag_info(flag_str_short);
                if flag_info.is_err() {
                    continue;
                }
                let (flag_x, flag_y, flag_type, flag_str) = flag_info.unwrap();
                let flag_position = Vector2f::new(room_x as f32 + flag_x + 0.5, room_y as f32 + flag_y + 0.5);
                let (tex_w, tex_h, tex) = user_tex_source.get_texture(flag_type as u64);
                let mut spr_flag = graphics::Sprite::with_texture(tex);
                spr_flag.set_origin(Vector2f::new(tex_w / 2.0, tex_h / 2.0));
                spr_flag.set_position(flag_position * 8.0);
                spr_flag.set_scale(8.0 / tex_w);

                if spr_flag.global_bounds().contains2(local_mouse_x, local_mouse_y) {
                    spr_flag.scale(1.3);

                    info_overlay_opt = Some(flag_str.to_string());

                    if sidebar_selection.is_none() && is_mouse_clicked.is_some_and(|bt| bt == mouse::Button::Left) {
                        spoiler_type = SpoilerType::Flag(i);
                        click_consumed = true;
                    }
                }

                window.draw_with_renderstates(&spr_flag, &states);
            }
        
            // Draw Door hover
            if sidebar_selection.is_some_and(|x| x.to_door_type().is_some()) && tile_hovered_opt.is_some() {
                let tile = tile_hovered_opt.unwrap();
                let door_type = sidebar_selection.unwrap();
                let tr = (local_mouse_x / 8.0).fract() > (local_mouse_y / 8.0).fract();
                let br = (local_mouse_x / 8.0).fract() > 1.0 - (local_mouse_y / 8.0).fract();
                let direction = (if tr && br { "right" } else if tr && !br { "up" } else if !tr && br { "down" } else { "left" }).to_string();

                let room_idx = plando.room_id_to_idx(tile.room_id);
                let door_idx_opt = plando.game_data.room_geometry[room_idx].doors.iter().position(
                    |x| x.direction == direction && x.x == tile.tile_x && x.y == tile.tile_y
                );
                if let Some(door_idx) = door_idx_opt {
                    let (room_x, room_y) = plando.map.rooms[room_idx];
                    let x = (room_x + tile.tile_x) as f32;
                    let y = (room_y + tile.tile_y) as f32;

                    let (_tex_w, _tex_h, tex) = user_tex_source.get_texture(door_type as u64);
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

                    if let Some(bt) = is_mouse_clicked {
                        let mut res = Ok(());
                        if bt == mouse::Button::Left {
                            res = plando.place_door(room_idx, door_idx, door_type.to_door_type(), false, false);
                        } else if bt == mouse::Button::Right {
                            res = plando.place_door(room_idx, door_idx, None, true, false);
                        }
                        click_consumed = true;
                        if let Err(err) = res {
                            error_modal_message = Some(err.to_string());
                        }
                    }

                    window.draw_with_renderstates(&spr_ghost, &states);
                }
            }

            // Draw the info overlay
            if let Some(info_overlay) = info_overlay_opt {
                let mut text = graphics::Text::new(&info_overlay, &font_default, 16);
                text.set_fill_color(graphics::Color::WHITE);
                text.set_position(Vector2f::new(mouse_x as f32 + 16.0, mouse_y as f32));
                let mut bg_rect = graphics::RectangleShape::new();
                bg_rect.set_position(Vector2f::new(mouse_x as f32 + 12.0, mouse_y as f32));
                bg_rect.set_size(Vector2f::new(text.global_bounds().size().x + 8.0, 24.0));
                bg_rect.set_fill_color(graphics::Color::rgba(0x1F, 0x1F, 0x1F, 0xBF));

                window.draw(&bg_rect);
                window.draw(&text);
            }

            // Draw spoiler route
            if spoiler_type != SpoilerType::None && plando.randomization.is_some() {
                let (_r, spoiler_log) = plando.randomization.as_ref().unwrap();
                let mut obtain_route = None;
                let mut return_route = None;
                let mut show_escape_route = false;

                match spoiler_type {
                    SpoilerType::Hub => {
                        obtain_route = Some(&spoiler_log.hub_obtain_route);
                        return_route = Some(&spoiler_log.hub_return_route);
                        spoiler_step = 0;
                    }
                    SpoilerType::Item(spoiler_idx) => {
                        let (room_id, node_id) = plando.game_data.item_locations[spoiler_idx];
                        let mut details_opt = None;
                        while details_opt.is_none() {
                            if spoiler_step < spoiler_log.details.len() {
                                details_opt = spoiler_log.details[spoiler_step].items.iter().find(
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
                                spoiler_step = step_opt.unwrap();
                            }
                        }
                        if let Some(details) = details_opt {
                            obtain_route = Some(&details.obtain_route);
                            return_route = Some(&details.return_route);
                        } else {
                            error_modal_message = Some("Item is nothing or not logically bireachable".to_string());
                            spoiler_type = SpoilerType::None;
                        }
                    }
                    SpoilerType::Flag(spoiler_idx) => {
                        let flag_id = plando.game_data.flag_ids[spoiler_idx];
                        let flag_name = &plando.game_data.flag_isv.keys[flag_id];
                        
                        let mut details_opt = None;
                        while details_opt.is_none() {
                            if spoiler_step < spoiler_log.details.len() {
                                details_opt = spoiler_log.details[spoiler_step].flags.iter().find(
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
                                spoiler_step = step_opt.unwrap();
                            }
                        }
                        if let Some(details) = details_opt {
                            obtain_route = Some(&details.obtain_route);
                            return_route = Some(&details.return_route);
                            show_escape_route = flag_id == plando.game_data.mother_brain_defeated_flag_id;
                        } else {
                            error_modal_message = Some("Flag not logically clearable".to_string());
                            spoiler_type = SpoilerType::None;
                        }
                    }
                    _ => {
                        spoiler_step = spoiler_log.details.len();
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

                    draw_thick_line_strip(&mut *window, &states, &vertex_escape, 1.0);
                    draw_thick_line_strip(&mut *window, &states, &vertex_return, 1.0);
                    draw_thick_line_strip(&mut *window, &states, &vertex_obtain, 1.0);
                }
            }

        }

        if !click_consumed && is_mouse_clicked.is_some() {
            sidebar_selection = None;
            spoiler_type = SpoilerType::None;
        }
        if sidebar_selection.is_some() {
            spoiler_type = SpoilerType::None;
        }

        // Draw Menu Bar
        let gui = sfegui.run(&mut window, |rt, ctx| {
            egui::TopBottomPanel::top("menu_file_main").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Save Seed").clicked() {
                            let file_opt = FileDialog::new()
                                .set_directory("/")
                                .add_filter("JSON File", &["json"])
                                .save_file();
                            if let Some(file) = file_opt {
                                let res = save_seed(&plando, file.as_path());
                                if res.is_err() {
                                    error_modal_message = Some(res.unwrap_err().to_string());
                                }
                            }
                            ui.close_menu();
                        }
                        if ui.button("Load Seed").clicked() {
                            let file_opt = FileDialog::new()
                                .set_directory("/")
                                .add_filter("JSON File", &["json"])
                                .pick_file();
                            if let Some(file) = file_opt {
                                let res = load_seed(&mut plando, file.as_path());
                                if res.is_err() {
                                    error_modal_message = Some(res.unwrap_err().to_string());
                                }
                            }
                            ui.close_menu();
                        }
                        if ui.button("Load Logic Preset from file").clicked() {
                            let file_opt = FileDialog::new()
                                .set_directory("/")
                                .add_filter("JSON File", &["json"])
                                .pick_file();
                            if let Some(file) = file_opt {
                                let res = plando.load_preset_from_file(&file);
                                if res.is_err() {
                                    error_modal_message = Some(res.unwrap_err().to_string());
                                }
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Create ROM").clicked() {
                            customize_open = true;
                            ui.close_menu();
                        }
                        if ui.button("Create ROM from seed file").clicked() {
                            if let Some(file) = FileDialog::new()
                            .set_directory("/").add_filter("JSON File", &["json"]).pick_file() {
                                match load_seed(&mut plando, &file) {
                                    Ok(_) => {
                                        customize_open = true;
                                        reset_after_patch = true;
                                    }
                                    Err(err) => error_modal_message = Some(err.to_string())
                                }
                                ui.close_menu();
                            }
                        }
                    });
                    ui.menu_button("Map", |ui| {
                        if ui.button("Reroll Map (Vanilla)").clicked() {
                            plando.reroll_map(MapRepositoryType::Vanilla).unwrap();
                            ui.close_menu();
                        }
                        if ui.add_enabled(plando.maps_standard.is_some(), egui::Button::new("Reroll Map (Standard)")).clicked() {
                            plando.reroll_map(MapRepositoryType::Standard).unwrap();
                            ui.close_menu();
                        }
                        if ui.add_enabled(plando.maps_wild.is_some(), egui::Button::new("Reroll Map (Wild)")).clicked() {
                            plando.reroll_map(MapRepositoryType::Wild).unwrap();
                            ui.close_menu();
                        }
                        if ui.button("Load Map from JSON").clicked() {
                            let file_opt = FileDialog::new()
                                .set_directory("/")
                                .add_filter("JSON File", &["json"])
                                .pick_file();
                            if let Some(file) = file_opt {
                                let res = load_map(&mut plando, &file);
                                if res.is_err() {
                                    error_modal_message = Some(res.unwrap_err().to_string());
                                }
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.add_enabled(status_message.is_none(), egui::Button::new("Download Map Repositories")).clicked() {
                            download_thread_handle = thread::spawn(download_map_repos);
                            download_thread_active = true;
                            status_message = Some("Downloading/Unpacking... This might take a while");
                        }
                    });
                    ui.menu_button("Items", |ui| {
                        if ui.button("Clear all Items").clicked() {
                            plando.clear_item_locations();
                        }
                        if ui.button("Clear all Doors").clicked() {
                            plando.clear_doors();
                        }
                        ui.separator();
                        if ui.button("Replace Nothings with Missiles").clicked() {
                            let auto_update = plando.auto_update_spoiler;
                            plando.auto_update_spoiler = false;
                            for i in 0..plando.item_locations.len() {
                                if plando.item_locations[i] == Item::Nothing {
                                    let _ = plando.place_item(i, Item::Missile);
                                }
                            }
                            plando.auto_update_spoiler = auto_update;
                            plando.update_spoiler_data();
                            ui.close_menu();
                        }
                        if ui.button("Randomize Doors").clicked() {
                            let update = plando.auto_update_spoiler;
                            plando.auto_update_spoiler = false;
                            plando.clear_doors();

                            let seed = (plando.rng.next_u64() & 0xFFFFFFFF) as usize;
                            let locked_door_data = maprando::randomize::randomize_doors(&plando.game_data, &plando.map, &plando.randomizer_settings, &plando.objectives, seed);
                            for door in locked_door_data.locked_doors {
                                let (room_idx, door_idx) = plando.game_data.room_and_door_idxs_by_door_ptr_pair[&door.src_ptr_pair];
                                let _ = plando.place_door(room_idx, door_idx, Some(door.door_type), false, true);
                            }

                            if let Err(_) = plando.update_hub_location() {
                                let _ = plando.place_start_location(Plando::get_ship_start());
                                error_modal_message = Some("The logical hub was blocked. Start location is defaulted to ship".to_string());
                            }

                            plando.auto_update_spoiler = update;
                            if update {
                                plando.update_spoiler_data();
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
                    })
                    
                });
            });

            // Draw item selection sidebar
            sidebar_width = egui::SidePanel::right("panel_item_select").resizable(false).show(ctx, |ui| {
                egui::scroll_area::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("grid_item_select")
                    .with_row_color(move |val, _style| {
                        if sidebar_selection.is_some_and(|x| x == Placeable::VALUES[val]) { Some(Color32::from_rgb(255, 0, 0)) } else { None }
                    }).min_row_height(sidebar_height).show(ui, |ui| {
                        for (row, placeable) in Placeable::VALUES.iter().enumerate() {
                            // If settigs don't allow ammo or beam doors, we don't allow their placement
                            if (*placeable >= Placeable::DoorMissile && plando.randomizer_settings.doors_mode == DoorsMode::Blue)
                                || (*placeable >= Placeable::DoorCharge && plando.randomizer_settings.doors_mode == DoorsMode::Ammo) {
                                break;
                            }

                            // Load image
                            let img = egui::Image::new(user_tex_source.get_image_source(*placeable as u64)).sense(Sense::click())
                                .fit_to_exact_size(Vec2::new(sidebar_height, sidebar_height));
                            let img_resp = ui.add(img);
                            if img_resp.clicked() {
                                sidebar_selection = Some(Placeable::VALUES[row]);
                            }

                            let item_count = plando.placed_item_count[row];
                            let max_item_count = plando.get_max_placeable_count(placeable.clone());

                            let label_name = egui::Label::new(placeable.to_string());
                            if ui.add(label_name).clicked() {
                                sidebar_selection = Some(Placeable::VALUES[row]);
                            }
                            let label_count_str = if max_item_count.is_some() { format!("{item_count} / {}", max_item_count.unwrap()) } else { item_count.to_string() };
                            let label_count = egui::Label::new(label_count_str).sense(Sense::click());
                            if ui.add(label_count).clicked() {
                                sidebar_selection = Some(Placeable::VALUES[row]);
                            }
                            // So it doesn't create an empty row at the very end
                            if row + 1 < Placeable::VALUES.len() {
                                ui.end_row();
                            }
                        }
                    });
                });
            }).response.rect.width();

            // Draw Spoiler Details Window
            if spoiler_type != SpoilerType::None && plando.randomization.is_some() {
                let (_r, spoiler_log) = plando.randomization.as_ref().unwrap();
                let window = egui::Window::new("Spoiler Details")
                    .resizable(false)
                    .title_bar(false)
                    .movable(false)
                    .vscroll(false)
                    .fixed_pos(Vec2::new(16.0, 32.0).to_pos2())
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.style_mut().spacing.item_spacing = Vec2::new(2.0, 2.0);
                            let details = &spoiler_log.details[spoiler_step];
                            ui.heading(format!("STEP {}", details.step));
                            ui.label("PREVIOUSLY COLLECTIBLE");

                            let mut collectible_items = [0; ITEM_VALUES.len() - 1];
                            let mut collectible_flags = vec![false; plando.game_data.flag_ids.len()];
                            for prev_step in 0..spoiler_step {
                                let prev_details = &spoiler_log.details[prev_step];
                                for item_details in &prev_details.items {
                                    let item_id = plando.game_data.item_isv.index_by_key[&item_details.item];
                                    collectible_items[item_id] += 1;
                                }
                                for flag_details in &prev_details.flags {
                                    let flag_id = plando.game_data.flag_isv.index_by_key[&flag_details.flag];
                                    let flag_idx = plando.game_data.flag_ids.iter().position(|x| *x == flag_id).unwrap();
                                    collectible_flags[flag_idx] = true;
                                }
                            }

                            let mut new_spoiler_type = spoiler_type.clone();

                            let minor_indices = [Item::Missile as usize, Item::Super as usize, Item::PowerBomb as usize, Item::ETank as usize, Item::ReserveTank as usize];
                            // Render Minor Items
                            ui.horizontal(|ui| {
                                for i in 0..minor_indices.len() {
                                    let idx = minor_indices[i];
                                    if collectible_items[idx] == 0 {
                                        continue;
                                    }
                                    let placeable_idx = Placeable::ETank as u64 + idx as u64;
                                    let img = egui::Image::new(user_tex_source.get_image_source(placeable_idx)).fit_to_exact_size(Vec2::new(16.0, 16.0));
                                    ui.add(img);
                                    let ammo_collected = (collectible_items[idx] as f32 * plando.randomizer_settings.item_progression_settings.ammo_collect_fraction).round() as i32 * 5;
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
                                    let img = egui::Image::new(user_tex_source.get_image_source(placeable_idx)).fit_to_exact_size(Vec2::new(16.0, 16.0));
                                    ui.add(img);
                                }
                            });
                            // Render Flags
                            ui.horizontal(|ui| {
                                for i in 0..collectible_flags.len() {
                                    let flag_id = plando.game_data.flag_ids[i];
                                    if !collectible_flags[i] || !flag_has_tex.contains(&flag_id) {
                                        continue;
                                    }
                                    let flag_tex_idx = FLAG_TEX_START + flag_id as u64;
                                    let img = egui::Image::new(user_tex_source.get_image_source(flag_tex_idx)).fit_to_exact_size(Vec2::new(24.0, 24.0)).sense(Sense::click());
                                    let resp = ui.add(img);
                                    if resp.clicked() {
                                        new_spoiler_type = SpoilerType::Flag(i);
                                    }
                                }
                            });

                            ui.label("COLLECTIBLE ON THIS STEP");
                            // Items
                            ui.horizontal(|ui| {
                                for item_details in &details.items {
                                    let item_id = plando.game_data.item_isv.index_by_key[&item_details.item];
                                    let placeable_id = Placeable::ETank as u64 + item_id as u64;
                                    let img = egui::Image::new(user_tex_source.get_image_source(placeable_id)).fit_to_exact_size(Vec2::new(16.0, 16.0)).sense(Sense::click());
                                    if ui.add(img).clicked() {
                                        let item_idx = plando.game_data.item_locations.iter().position(
                                            |x| x.0 == item_details.location.room_id && x.1 == item_details.location.node_id
                                        ).unwrap();
                                        new_spoiler_type = SpoilerType::Item(item_idx);
                                    }
                                }
                            });
                            // Flags
                            ui.horizontal(|ui| {
                                for flag_details in &details.flags {
                                    let flag_id = plando.game_data.flag_isv.index_by_key[&flag_details.flag];
                                    if !flag_has_tex.contains(&flag_id) {
                                        continue;
                                    }
                                    let flag_tex_id = FLAG_TEX_START + flag_id as u64;
                                    let img = egui::Image::new(user_tex_source.get_image_source(flag_tex_id)).fit_to_exact_size(Vec2::new(24.0, 24.0)).sense(Sense::click());
                                    if ui.add(img).clicked() {
                                        let flag_idx = plando.game_data.flag_ids.iter().position(
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

                            match spoiler_type {
                                SpoilerType::Item(item) => {
                                    let (room_id, node_id) = plando.game_data.item_locations[item];
                                    let item_details = details.items.iter().find(
                                        |x| x.location.room_id == room_id && x.location.node_id == node_id
                                    ).unwrap();
                                    details_name = item_details.item.clone() + item_details.difficulty.as_ref().unwrap_or(&String::new());
                                    details_location = item_details.location.room.clone() + ": " + &item_details.location.node;
                                    details_area = item_details.location.area.clone();
                                    details_obtain_route = &item_details.obtain_route;
                                    details_return_route = &item_details.return_route;
                                }
                                SpoilerType::Flag(flag_idx) => {
                                    let flag_id = plando.game_data.flag_ids[flag_idx];
                                    let flag_name = plando.game_data.flag_isv.keys[flag_id].clone();
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
                                    details_name = "Hub Route".to_string();
                                    details_location = plando.start_location_data.hub_location.name.clone();
                                    details_area = String::new();
                                    details_obtain_route = &plando.start_location_data.hub_obtain_route;
                                    details_return_route = &plando.start_location_data.hub_return_route;
                                }
                            };
                            ui.heading(details_name);
                            ui.label("LOCATION");
                            ui.label(details_location);
                            ui.label(details_area);

                            ui.separator();
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

                            spoiler_type = new_spoiler_type;
                        });
                    }).unwrap().response;
                if window.contains_pointer() {
                    spoiler_details_hovered = true;
                }
            } else if plando.randomization.is_some() {
                // Draw Spoiler Summary
                let resp_opt = egui::Window::new("Spoiler Summary")
                .resizable(false).movable(false).title_bar(false).min_width(320.0)
                .fixed_pos(Vec2::new(16.0, 32.0).to_pos2()).show(ctx, |ui| {
                    let (_r, spoiler_log) = plando.randomization.as_ref().unwrap();
                    let ui_builder = egui::UiBuilder::new().sense(Sense::click());

                    let resp = ui.scope_builder(ui_builder, |ui| {
                        egui::Grid::new("spoiler_summary").num_columns(2).with_row_color(move |row, _style| {
                            if row != spoiler_step { Some(Color32::from_rgb(0x20, 0x20, 0x20)) } else { Some(Color32::from_rgb(0x40, 0x40, 0x74)) }
                        }).show(ui, |ui| {
                            ui.style_mut().spacing.item_spacing = Vec2::new(2.0, 2.0);
                            let mut items_found = [false; ITEM_VALUES.len()];
                            for summary in &spoiler_log.summary {
                                ui.label(summary.step.to_string());
                                ui.horizontal_wrapped(|ui| {
                                    for item_summary in &summary.items {
                                        let item = plando.game_data.item_isv.index_by_key[&item_summary.item];
                                        if items_found[item] {
                                            continue;
                                        }
                                        items_found[item] = true;
                                        let placeable_id = Placeable::ETank as u64 + item as u64;
                                        let img = egui::Image::new(user_tex_source.get_image_source(placeable_id))
                                            .fit_to_exact_size(Vec2::new(16.0, 16.0)).sense(Sense::click());
                                        if ui.add(img).clicked() {
                                            spoiler_step = summary.step - 1;
                                            let item_loc = plando.game_data.item_locations.iter().position(
                                                |x| x.0 == item_summary.location.room_id && x.1 == item_summary.location.node_id
                                            ).unwrap();
                                            spoiler_type = SpoilerType::Item(item_loc);
                                        }
                                    }
                                });
                                ui.end_row();
                            }
                        });
                    }).response;
                    
                    let local_my = mouse_y as f32 - spoiler_window_bounds.top;
                    let idx_unchecked = (spoiler_log.summary.len() as f32 * local_my / spoiler_window_bounds.height).floor() as i32;
                    let idx = min(max(idx_unchecked, 0), spoiler_log.summary.len() as i32 - 1) as usize;
                    if resp.clicked() {
                        spoiler_step = idx;
                    }
                    if resp.double_clicked() && !spoiler_log.summary[idx].items.is_empty() {
                        spoiler_step = idx;
                        let item_loc = &spoiler_log.summary[idx].items[0].location;
                        let item = plando.game_data.item_locations.iter().position(
                            |x| x.0 == item_loc.room_id && x.1 == item_loc.node_id
                        ).unwrap();
                        spoiler_type = SpoilerType::Item(item);
                    }
                }).map(|x| x.response);
                if let Some(resp) = resp_opt {
                    spoiler_window_bounds = FloatRect::new(resp.rect.left(), resp.rect.top(), resp.rect.width(), resp.rect.height());
                }
            }

            if settings_open {
                egui::Window::new("Settings")
                .resizable(false)
                .title_bar(false)
                .show(ctx, |ui| {
                    egui::Grid::new("grid_settings").num_columns(3).striped(true).show(ui, |ui| {
                        let default = Settings::default();
                        
                        ui.label("Click delay tolerance").on_hover_text("Time in frames between a click and release to count as a click and not a drag");
                        ui.add(egui::DragValue::new(&mut settings.mouse_click_delay_tolerance).range(1..=600));
                        if ui.add_enabled(settings.mouse_click_delay_tolerance != default.mouse_click_delay_tolerance, egui::Button::new("Reset")).clicked() {
                            settings.mouse_click_delay_tolerance = default.mouse_click_delay_tolerance;
                        }
                        ui.end_row();

                        ui.label("Click position tolerance").on_hover_text("Distance in pixels between a mouse click and release to count as a click and not a drag");
                        ui.add(egui::DragValue::new(&mut settings.mouse_click_pos_tolerance).range(1..=600));
                        if ui.add_enabled(settings.mouse_click_pos_tolerance != default.mouse_click_pos_tolerance, egui::Button::new("Reset")).clicked() {
                            settings.mouse_click_pos_tolerance = default.mouse_click_pos_tolerance;
                        }
                        ui.end_row();

                        ui.label("ROM Path").on_hover_text("Path to your vanilla Super Metroid ROM");
                        let mut rom_path_text = if settings.rom_path.is_empty() { "Empty".to_string() } else { settings.rom_path.clone() };
                        let old_str_len = rom_path_text.len();
                        rom_path_text.truncate(32);
                        if old_str_len > 32 {
                            rom_path_text += "...";
                        }
                        if ui.button(rom_path_text).clicked() {
                            let file_opt = FileDialog::new().add_filter("Snes ROM", &["smc", "sfc"]).set_directory("/").pick_file();
                            if let Some(file) = file_opt {
                                settings.rom_path = file.to_str().unwrap().to_string();
                                match load_vanilla_rom(Path::new(&settings.rom_path)) {
                                    Ok(rom) => rom_vanilla = Some(rom),
                                    Err(err) => error_modal_message = Some(err.to_string())
                                }
                            }
                        }
                        ui.end_row();

                        ui.label("Auto update Spoiler").on_hover_text("If checked will automatically update the spoiler after each change. Does affect performance. Press F5 to manually update spoiler");
                        ui.checkbox(&mut settings.spoiler_auto_update, "Auto update Spoiler");
                        if ui.add_enabled(settings.spoiler_auto_update != default.spoiler_auto_update, egui::Button::new("Reset")).clicked() {
                            settings.spoiler_auto_update = default.spoiler_auto_update;
                        }
                        plando.auto_update_spoiler = settings.spoiler_auto_update;
                        ui.end_row();

                        ui.label("Disable logic").on_hover_text("Does not gray out unreachable locations. Useful for modelling out-of-logic");
                        ui.checkbox(&mut settings.disable_logic, "Disable logic");
                        if ui.add_enabled(settings.disable_logic != default.disable_logic, egui::Button::new("Reset")).clicked() {
                            settings.disable_logic = default.disable_logic;
                        }
                        ui.end_row();

                        ui.label("Disable background grid").on_hover_text("Disables the grid of dotted lines making up the background");
                        ui.checkbox(&mut settings.disable_bg_grid, "Disable background grid");
                        if ui.add_enabled(settings.disable_bg_grid != default.disable_bg_grid, egui::Button::new("Reset")).clicked() {
                            settings.disable_bg_grid = default.disable_bg_grid;
                        }
                        ui.end_row();

                        ui.label("Auto Updates").on_hover_text("Checks github for latest releases and updates to newer versions if possible");
                        ui.checkbox(&mut settings.auto_update, "Check for update on Startup");
                        if ui.add_enabled(settings.auto_update != default.auto_update, egui::Button::new("Reset")).clicked() {
                            settings.auto_update = default.auto_update;
                        }
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            if let Err(err) = save_settings(&settings, settings_path) {
                                error_modal_message = Some(err.to_string());
                            }
                            settings_open = false;
                        }
                        if ui.button("Reset All").clicked() {
                            settings = Settings::default();
                        }
                    });
                });
            }

            if customize_open {
                egui::Window::new("Customize")
                .resizable(false)
                .title_bar(false)
                .show(ctx, |ui| {
                    egui::Grid::new("grid_customize").num_columns(2).striped(true).show(ui, |ui| {
                        ui.label("Samus sprite");
                        egui::ComboBox::from_id_salt("combo_customize").selected_text(&settings.customization.samus_sprite).show_ui(ui, |ui| {
                            for category in &plando.samus_sprite_categories {
                                for sprite in &category.sprites {
                                    ui.selectable_value(&mut settings.customization.samus_sprite, sprite.name.clone(), sprite.display_name.clone());
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("Energy tank color");
                        ui.color_edit_button_rgb(&mut settings.customization.etank_color);
                        ui.end_row();

                        ui.label("Door colors");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.door_theme, 0, "Vanilla");
                            ui.selectable_value(&mut settings.customization.door_theme, 1, "Alternate");
                        });
                        ui.end_row();

                        ui.label("Music");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.music, 0, "On");
                            ui.selectable_value(&mut settings.customization.music, 1, "Off");
                        });
                        ui.end_row();

                        ui.label("Screen shaking");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.shaking, 0, "Vanilla");
                            ui.selectable_value(&mut settings.customization.shaking, 1, "Reduced");
                            ui.selectable_value(&mut settings.customization.shaking, 2, "Disabled");
                        });
                        ui.end_row();

                        ui.label("Screen flashing");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.flashing, 0, "Vanilla");
                            ui.selectable_value(&mut settings.customization.flashing, 1, "Reduced");
                        });
                        ui.end_row();

                        ui.label("Low-energy beeping");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.disable_beeping, false, "Vanilla");
                            ui.selectable_value(&mut settings.customization.disable_beeping, true, "Disabled");
                        });
                        ui.end_row();

                        ui.separator();
                        ui.end_row();

                        ui.label("Room palettes");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.palette_theme, 0, "Vanilla");
                            ui.selectable_value(&mut settings.customization.palette_theme, 1, "Area-themed");
                        });
                        ui.end_row();

                        ui.label("Tile theme");
                        let mut tile_theme_strs: Vec<String> = vec!["Vanilla", "Area-themed", "Scrambled"].iter().map(|x| x.to_string()).collect();
                        plando.mosaic_themes.iter().for_each(|x| tile_theme_strs.push(x.display_name.clone()));
                        tile_theme_strs.push("Practice Outlines".to_string());
                        tile_theme_strs.push("Invisible".to_string());
                        egui::ComboBox::from_id_salt("combo_customize_tile").selected_text(&tile_theme_strs[settings.customization.tile_theme]).show_ui(ui, |ui| {
                            for (i, theme) in tile_theme_strs.iter().enumerate() {
                                ui.selectable_value(&mut settings.customization.tile_theme, i, theme);
                            }
                        });
                        ui.end_row();

                        ui.label("Reserve tank HUD style");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.reserve_hud_style, false, "Vanilla");
                            ui.selectable_value(&mut settings.customization.reserve_hud_style, true, "Revamped");
                        });
                        ui.end_row();

                        ui.label("Screw Attack animation");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.vanilla_screw_attack_animation, true, "Vanilla");
                            ui.selectable_value(&mut settings.customization.vanilla_screw_attack_animation, false, "Split");
                        });
                        ui.end_row();

                        use CustomControllerButton::*;
                        const VALUES: [CustomControllerButton; 12] = [X, Y, A, B, L, R, Select, Start, Up, Down, Left, Right];
                        const STRINGS: [&str; 12] = ["X", "Y", "A", "B", "L", "R", "Select", "Start", "Up", "Down", "Left", "Right"];
                        let config = &mut settings.customization.controller_config;

                        ui.label("Shot");
                        ui.horizontal(|ui| {
                            for i in 0..7 {
                                ui.selectable_value(&mut config.shot, VALUES[i], STRINGS[i]);
                            }
                        });
                        ui.end_row();

                        ui.label("Jump");
                        ui.horizontal(|ui| {
                            for i in 0..7 {
                                ui.selectable_value(&mut config.jump, VALUES[i], STRINGS[i]);
                            }
                        });
                        ui.end_row();

                        ui.label("Dash");
                        ui.horizontal(|ui| {
                            for i in 0..7 {
                                ui.selectable_value(&mut config.dash, VALUES[i], STRINGS[i]);
                            }
                        });
                        ui.end_row();

                        ui.label("Item Select");
                        ui.horizontal(|ui| {
                            for i in 0..7 {
                                ui.selectable_value(&mut config.item_select, VALUES[i], STRINGS[i]);
                            }
                        });
                        ui.end_row();

                        ui.label("Item Cancel");
                        ui.horizontal(|ui| {
                            for i in 0..7 {
                                ui.selectable_value(&mut config.item_cancel, VALUES[i], STRINGS[i]);
                            }
                        });
                        ui.end_row();

                        ui.label("Angle Up");
                        ui.horizontal(|ui| {
                            for i in 0..7 {
                                ui.selectable_value(&mut config.angle_up, VALUES[i], STRINGS[i]);
                            }
                        });
                        ui.end_row();

                        ui.label("Angle Down");
                        ui.horizontal(|ui| {
                            for i in 0..7 {
                                ui.selectable_value(&mut config.angle_down, VALUES[i], STRINGS[i]);
                            }
                        });
                        ui.end_row();

                        ui.label("Quick reload");
                        ui.horizontal(|ui| {
                            for i in 0..VALUES.len() {
                                let resp = ui.selectable_label(config.quick_reload_buttons.contains(&VALUES[i]), STRINGS[i]);
                                if resp.clicked() {
                                    if let Some(pos) = config.quick_reload_buttons.iter().position(|x| *x == VALUES[i]) {
                                        config.quick_reload_buttons.remove(pos);
                                    } else {
                                        config.quick_reload_buttons.push(VALUES[i]);
                                    }
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("Spin lock");
                        ui.horizontal(|ui| {
                            for i in 0..VALUES.len() {
                                let resp = ui.selectable_label(config.spin_lock_buttons.contains(&VALUES[i]), STRINGS[i]);
                                if resp.clicked() {
                                    if let Some(pos) = config.spin_lock_buttons.iter().position(|x| *x == VALUES[i]) {
                                        config.spin_lock_buttons.remove(pos);
                                    } else {
                                        config.spin_lock_buttons.push(VALUES[i]);
                                    }
                                }
                            }
                        });
                        ui.end_row();

                        ui.label("Moonwalk");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut settings.customization.controller_config.moonwalk, false, "No");
                            ui.selectable_value(&mut settings.customization.controller_config.moonwalk, true, "Yes");
                        });
                        ui.end_row();

                        ui.separator();
                        ui.end_row();

                        while ui.button("Patch ROM").clicked() {
                            if !settings.customization.controller_config.is_valid() {
                                error_modal_message = Some("Controller config is invalid".to_string());
                                break;
                            }
                            if rom_vanilla.is_none() {
                                if let Some(file) = FileDialog::new().set_title("Select vanilla ROM")
                                .set_directory("/").add_filter("Snes ROM", &["sfc", "smc"]).pick_file() {
                                    settings.rom_path = file.to_str().unwrap().to_string();
                                    match load_vanilla_rom(&Path::new(&settings.rom_path)) {
                                        Ok(rom) => rom_vanilla = Some(rom),
                                        Err(err) => error_modal_message = Some(err.to_string())
                                    }
                                }
                            }
                            if rom_vanilla.is_none() {
                                break;
                            }
                            let rom = rom_vanilla.as_ref().unwrap();
                            if let Some(file_out) = FileDialog::new().set_title("Select output location")
                            .set_directory("/")
                            .add_filter("Snes ROM", &["sfc"])
                            .save_file() {
                                let customize_settings = settings.customization.to_settings(&plando.mosaic_themes);
                                if let Err(err) = patch_rom(&mut plando, rom, &customize_settings, &file_out) {
                                    error_modal_message = Some(err.to_string());
                                }
                                customize_open = false;
                            }

                            if reset_after_patch {
                                let _ = plando.reroll_map(MapRepositoryType::Vanilla);
                                reset_after_patch = false;
                            }
                            break;
                        }
                        if ui.button("Cancel").clicked() {
                            customize_open = false;
                            if reset_after_patch {
                                let _ = plando.reroll_map(MapRepositoryType::Vanilla);
                                reset_after_patch = false;
                            }
                        }
                    });
                });
            }

            if customize_logic_open {
                egui::Window::new("Customize Logic").resizable(false).title_bar(false).show(ctx, |ui| {
                    // Settings preset
                    ui.horizontal(|ui| {
                        ui.label("Settings preset");
                        let combo_text = match &cur_settings.name {
                            None => "Select a preset to automatically fill all settings".to_string(),
                            Some(name) => name.clone()
                        };
                        egui::ComboBox::from_id_salt("combo_logic_preset").selected_text(combo_text).show_ui(ui, |ui| {
                            if ui.selectable_label(cur_settings.name.is_none(), "Select a preset to automatically fill all settings").clicked() {
                                cur_settings.name = None;
                            }
                            ui.separator();
                            for preset in &plando.preset_data.full_presets {
                                if ui.selectable_label(cur_settings.name.as_ref().is_some_and(|x| *x == *preset.name.as_ref().unwrap()), preset.name.as_ref().unwrap().clone()).clicked() {
                                    cur_settings = preset.clone();
                                }
                            }
                        });
                    });
                    egui::Grid::new("grid_customize_logic").num_columns(9).striped(true).show(ui, |ui| {
                        // Skill assumptions
                        ui.label("Skill assumptions");
                        for preset in &plando.preset_data.skill_presets {
                            if ui.selectable_label(cur_settings.skill_assumption_settings == *preset, preset.preset.as_ref().unwrap()).clicked() {
                                cur_settings.skill_assumption_settings = preset.clone();
                            }
                        }
                        if ui.button("Custom").clicked() {
                            cur_customize_logic_window = CustomizeLogicWindow::SkillAssumption;
                        }
                        ui.end_row();

                        // Item progression
                        ui.label("Item Progression").on_hover_text("These presets are mostly visual (except for Desolate), as they affect item progression");
                        for preset in &plando.preset_data.item_progression_presets {
                            if ui.selectable_label(cur_settings.item_progression_settings == *preset, preset.preset.as_ref().unwrap()).clicked() {
                                cur_settings.item_progression_settings = preset.clone();
                            }
                        }
                        if ui.button("Custom").clicked() {
                            cur_customize_logic_window = CustomizeLogicWindow::ItemProgression;
                        }
                        ui.end_row();

                        // Quality of Life
                        ui.label("Quality-of-life options");
                        for preset in &plando.preset_data.quality_of_life_presets {
                            if ui.selectable_label(cur_settings.quality_of_life_settings == *preset, preset.preset.as_ref().unwrap()).clicked() {
                                cur_settings.quality_of_life_settings = preset.clone();
                            }
                        }
                        if ui.button("Custom").clicked() {
                            cur_customize_logic_window = CustomizeLogicWindow::Qol;
                        }
                        ui.end_row();

                        // Objectives
                        ui.label("Objectives");
                        for preset in &plando.preset_data.objective_presets {
                            if ui.selectable_label(cur_settings.objective_settings == *preset, preset.preset.as_ref().unwrap()).clicked() {
                                cur_settings.objective_settings = preset.clone();
                            }
                        }
                        if ui.button("Custom").clicked() {
                            cur_customize_logic_window = CustomizeLogicWindow::Objectives;
                        }
                        ui.end_row();

                        // Update objective count
                        let num_obj = cur_settings.objective_settings.objective_options.iter().filter(
                            |x| x.setting == ObjectiveSetting::Yes
                        ).count() as i32;
                        cur_settings.objective_settings.min_objectives = num_obj;
                        cur_settings.objective_settings.max_objectives = num_obj;

                        // Doors
                        ui.label("Doors");
                        ui.selectable_value(&mut cur_settings.doors_mode, DoorsMode::Blue, "Blue");
                        ui.selectable_value(&mut cur_settings.doors_mode, DoorsMode::Ammo, "Ammo");
                        ui.selectable_value(&mut cur_settings.doors_mode, DoorsMode::Beam, "Beam");
                        ui.end_row();

                        // Save the animals
                        ui.label("Save the animals");
                        ui.selectable_value(&mut cur_settings.save_animals, SaveAnimals::No, "No");
                        ui.selectable_value(&mut cur_settings.save_animals, SaveAnimals::Yes, "Yes");
                        ui.selectable_value(&mut cur_settings.save_animals, SaveAnimals::Optional, "Optional");
                        ui.end_row();

                        // Collectible Walljump
                        ui.label("Wall Jump");
                        ui.selectable_value(&mut cur_settings.other_settings.wall_jump, WallJump::Vanilla, "Vanilla");
                        ui.selectable_value(&mut cur_settings.other_settings.wall_jump, WallJump::Collectible, "Collectible");
                        ui.end_row();

                        // Item dots after collection
                        ui.label("Item dots after collection");
                        ui.selectable_value(&mut cur_settings.other_settings.item_dot_change, ItemDotChange::Fade, "Fade");
                        ui.selectable_value(&mut cur_settings.other_settings.item_dot_change, ItemDotChange::Disappear, "Disappear");
                        ui.end_row();

                        // Area transition markers
                        ui.label("Area transition markers on map");
                        ui.selectable_value(&mut cur_settings.other_settings.transition_letters, false, "Arrows");
                        ui.selectable_value(&mut cur_settings.other_settings.transition_letters, true, "Letters");
                        ui.end_row();

                        // Door locks size
                        ui.label("Door locks size on map");
                        ui.selectable_value(&mut cur_settings.other_settings.door_locks_size, DoorLocksSize::Small, "Small");
                        ui.selectable_value(&mut cur_settings.other_settings.door_locks_size, DoorLocksSize::Large, "Large");
                        ui.end_row();

                        // Maps revealed from start
                        ui.label("Maps revealed from start");
                        ui.selectable_value(&mut cur_settings.other_settings.maps_revealed, MapsRevealed::No, "No");
                        ui.selectable_value(&mut cur_settings.other_settings.maps_revealed, MapsRevealed::Partial, "Partial");
                        ui.selectable_value(&mut cur_settings.other_settings.maps_revealed, MapsRevealed::Full, "Full");
                        ui.end_row();

                        // Map station reveal
                        ui.label("Map station activation reveal");
                        ui.selectable_value(&mut cur_settings.other_settings.map_station_reveal, MapStationReveal::Partial, "Partial");
                        ui.selectable_value(&mut cur_settings.other_settings.map_station_reveal, MapStationReveal::Full, "Full");
                        ui.end_row();

                        // Energy free shinesparks
                        ui.label("Energy-free shinesparks");
                        ui.selectable_value(&mut cur_settings.other_settings.energy_free_shinesparks, false, "No");
                        ui.selectable_value(&mut cur_settings.other_settings.energy_free_shinesparks, true, "Yes");
                        ui.end_row();

                        // Ultra low qol
                        ui.label("Ultra-low quality of life");
                        ui.selectable_value(&mut cur_settings.other_settings.ultra_low_qol, false, "No");
                        if ui.selectable_label(cur_settings.other_settings.ultra_low_qol, "Yes").clicked() {
                            cur_settings.other_settings.ultra_low_qol = true;
                            cur_settings.quality_of_life_settings = plando.preset_data.quality_of_life_presets.iter().find(
                                |x| x.preset.as_ref().is_some_and(|x| *x == "Off".to_string())
                            ).unwrap().clone();
                        }
                    });
                    // Save preset
                    if cur_settings.name.is_none() {
                        cur_settings.name = Some(String::new());
                    }
                    ui.horizontal(|ui| {
                        ui.label("Save preset as");
                        ui.text_edit_singleline(cur_settings.name.as_mut().unwrap());
                    });
                    ui.end_row();

                    // Apply / Save / Cancel
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            plando.load_preset(cur_settings.clone());
                            settings.last_logic_preset = Some(cur_settings.clone());
                            customize_logic_open = false;
                        }
                        if ui.button("Save to file").clicked() && cur_settings.name.as_ref().is_some_and(|x| !x.is_empty()) {
                            if let Err(err) = save_preset(&cur_settings) {
                                error_modal_message = Some(err.to_string());
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            cur_settings = plando.randomizer_settings.clone();
                            customize_logic_open = false;
                        }
                    });
                });

                let height = rt.size().y as f32 * 0.9;
                match cur_customize_logic_window {
                    CustomizeLogicWindow::None => {}
                    CustomizeLogicWindow::SkillAssumption => {
                        layout::window_skill_assumptions(height, &mut is_customize_window_open, &mut cur_settings, &plando.preset_data, ctx);
                    }
                    CustomizeLogicWindow::ItemProgression => {
                        layout::window_item_progression(height, &mut is_customize_window_open, &mut cur_settings, ctx);
                    }
                    CustomizeLogicWindow::Qol => {
                        layout::window_qol(height, &mut is_customize_window_open, &mut cur_settings, ctx);
                    }
                    CustomizeLogicWindow::Objectives => {
                        layout::window_objectives(height, &mut is_customize_window_open, &mut cur_settings, ctx);
                    }
                };

                if !is_customize_window_open {
                    cur_customize_logic_window = CustomizeLogicWindow::None;
                    is_customize_window_open = true;
                }
            }

            if let Some(msg) = status_message.clone() {
                egui::Modal::new(Id::new("modal_status")).show(ctx, |ui| {
                    ui.set_min_width(256.0);
                    ui.heading("Status");
                    ui.label(msg);
                });
            }

            if let Some(err_msg) = error_modal_message.clone() {
                let modal = egui::Modal::new(Id::new("modal_error")).show(ctx, |ui| {
                    ui.set_min_width(256.0);
                    ui.heading("Error");
                    ui.label(err_msg);
                    if ui.button("OK").clicked() {
                        error_modal_message = None;
                    }
                });
                if modal.should_close() {
                    error_modal_message = None;
                }
            }
        }).unwrap();
        sfegui.draw(gui, &mut window, Some(&mut user_tex_source));
        window.display();
    }
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
        let tex = self.tex_map.get(&id).context("Invalid texture id provided").unwrap();
        (tex.size().x as f32, tex.size().y as f32, tex)
    }
}