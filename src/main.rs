use {
    anyhow::{bail, Result}, egui_sfml::{egui::{self, Id}, SfEgui}, hashbrown::HashMap, maprando::{
        map_repository::MapRepository, patch::Rom, preset::PresetData, randomize::{DifficultyConfig, Randomization, Randomizer}, settings::RandomizerSettings, traverse::LockedDoorData
    }, maprando_game::{GameData, LinksDataGroup, Map, MapTileEdge, MapTileInterior, MapTileSpecialType}, rand::{
        RngCore, SeedableRng
    }, sfml::{
        cpp::FBox, graphics::{
            self, Color, RectangleShape, RenderTarget, RenderTexture, RenderWindow, Shape, Text, Transformable
        }, system::Vector2f, window::{
            mouse, ContextSettings, Event, Style
        }
    }, std::{cmp::max, path::Path, u32}
};

struct Plando {
    game_data: GameData,
    room_data: Vec<RoomData>,
    atlas_tex: FBox<graphics::Texture>,
    maps_vanilla: MapRepository,
    maps_standard: MapRepository,
    maps_wild: MapRepository,
    map: Map
}

impl Plando {
    fn new() -> Self {
        let game_data = load_game_data().unwrap();
        let (atlas_image, room_data) = load_room_sprites(&game_data).unwrap();
        let atlas_tex = graphics::Texture::from_image(&atlas_image, graphics::Rect::default()).unwrap();

        let vanilla_map_path = Path::new("../maps/vanilla");
        let standard_maps_path = Path::new("../maps/v117c-standard");
        let wild_maps_path = Path::new("../maps/v117c-wild");
    
        let maps_vanilla = MapRepository::new("Vanilla", vanilla_map_path).unwrap();
        let maps_standard = MapRepository::new("Standard", standard_maps_path).unwrap();
        let maps_wild = MapRepository::new("Wild", wild_maps_path).unwrap();

        let map = roll_map(&maps_vanilla, &game_data).unwrap();

        Plando {
            game_data,
            room_data,
            atlas_tex,
            maps_vanilla,
            maps_standard,
            maps_wild,
            map
        }
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

fn load_preset_data(game_data: &GameData) -> Result<PresetData> {
    let tech_path = Path::new("data/tech_data.json");
    let notable_path = Path::new("data/notable_data.json");
    let presets_path = Path::new("data/presets.json");

    let preset_data = PresetData::load(tech_path, notable_path, presets_path, game_data);
    preset_data
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

fn load_preset(path: &Path) -> Result<RandomizerSettings> {
    let json_data = std::fs::read_to_string(path)?;
    let result = maprando::settings::parse_randomizer_settings(&json_data);
    result
}

#[derive(Clone)]
struct RoomData {
    room_idx: usize,
    room_name: String,
    tile_width: u32,
    tile_height: u32,
    atlas_x_offset: u32,
    atlas_y_offset: u32,
    double_item: Option<(u32, u32)>
}

enum SpecialRoom {
    EnergyRefill,
    AmmoRefill,
    FullRefill,
    SaveStation,
    MapStation    
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
            [1, 1, 1, 1, 1, 1, 1, 1]]
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

        let mut double_item: Option<(u32, u32)> = None;

        let mut image = graphics::Image::new_solid(8 * max_width as u32, 8 * max_height as u32, graphics::Color::TRANSPARENT)?;
        for tile in &map_tile_data.map_tiles {
            let x_offset = tile.coords.0 as u32 * 8;
            let y_offset = tile.coords.1 as u32 * 8;
            
            if let Some(water_level) = tile.water_level {
                let start_index = (water_level * 8.0).round() as u32;
                for y in start_index..8 {
                    for x in 0..8 {
                        if (x + y) % 2 == 0 {
                            image.set_pixel(x_offset + x, y_offset + y, graphics::Color::BLACK)?;
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
                            |x: i32| (7.0 - (x as f32) * 0.5).floor() as i32
                        } else {
                            |x: i32| (3.0 - (x as f32) * 0.5).floor() as i32
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
                if tile.interior == MapTileInterior::DoubleItem {
                    double_item = Some((tile.coords.0 as u32, tile.coords.1 as u32));
                }
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
            //room_id: map_tile_data.room_id,
            room_idx: room_idx,
            room_name: map_tile_data.room_name.clone(),
            tile_width: max_width as u32,
            tile_height: max_height as u32,
            atlas_x_offset: 0,
            atlas_y_offset: 0,
            double_item
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

    let (room_data, _): (Vec<_>, Vec<_>) = image_mappings.into_iter().unzip();

    Ok((atlas, room_data))
}

fn roll_map(repo: &MapRepository, game_data: &GameData) -> Result<Map> {
    let rng_seed = [0u8; 32];
    let mut rng = rand::rngs::StdRng::from_seed(rng_seed);

    let map_seed = (rng.next_u64() & 0xFFFFFFFF) as usize;
    repo.get_map(1, map_seed, game_data)
}

/*fn get_randomizer<'a>(map: &'a Map, settings: &'a RandomizerSettings, preset_data: PresetData, game_data: &'a GameData) -> Randomizer<'a> {
    let skill_settings = &settings.skill_assumption_settings;
    let item_settings = &settings.item_progression_settings;
    let qol_settings = &settings.quality_of_life_settings;
    let other_settings = &settings.other_settings;

    let implicit_tech = &preset_data.tech_by_difficulty["Implicit"];
    let implicit_notables = &preset_data.notables_by_difficulty["Implicit"];
    let difficulty = DifficultyConfig::new(
        &skill_settings,
        &game_data,
        &implicit_tech,
        &implicit_notables,
    );
    let difficulty_tiers = maprando::randomize::get_difficulty_tiers(
        &settings,
        &preset_data.difficulty_tiers,
        &game_data,
        &preset_data.tech_by_difficulty["Implicit"],
        &preset_data.notables_by_difficulty["Implicit"],
    );

    let filtered_base_links = maprando::randomize::filter_links(&game_data.links, &game_data, &difficulty);
    let filtered_base_links_data = LinksDataGroup::new(
        filtered_base_links,
        game_data.vertex_isv.keys.len(),
        0,
    );

    let mut rng = rand::rngs::StdRng::from_entropy();

    let objectives = maprando::randomize::get_objectives(settings, &mut rng);
    let locked_door_data = LockedDoorData {
        locked_doors: Vec::new(),
        locked_door_node_map: HashMap::new(),
        locked_door_vertex_ids: Vec::new()
    };

    Randomizer::new(map, &locked_door_data, objectives, settings, &difficulty_tiers, game_data, &filtered_base_links_data, &mut rng)
}*/

fn main() {

    let mut plando = Plando::new();

    //let rom_path = Path::new("C:/Users/Loptr/Desktop/Super Metroid/Original ROM/Super Metroid (JU) [!].smc");
    //let rom_original = load_vanilla_rom(rom_path).unwrap();

    let preset_path = Path::new("./data/presets/full-settings/Community Race Season 3 (No animals).json");
    let mut randomizer_settings = load_preset(preset_path).unwrap();

    let game_data = &plando.game_data;
    let atlas_tex = &plando.atlas_tex;
    let room_data = &plando.room_data;
    //let difficulty_tiers = maprando::randomize::get_difficulty_tiers(&randomizer_settings, game_data.p, &game_data, implicit_tech, implicit_notables);

    /*let randomization = Randomization {
        settings: randomizer_settings,
        difficulty: difficulty_tiers[0].clone(),
    };
    let rom_rando = maprando::patch::make_rom(&rom_original, &randomization, &game_data).unwrap();*/
    /*let randomizer = Randomizer::new(
        &map,
        locked_door_data,
        objectives,
        settings,
        difficulty_tiers,
        &game_data,
        base_links_data,
        rng
    );*/


    let mut window = RenderWindow::new((1080, 720), "Maprando Plando", Style::DEFAULT, &Default::default()).expect("Could not create Window");
    window.set_vertical_sync_enabled(true);

    let font_default = graphics::Font::from_file("./res/segoeui.ttf").expect("Could not load default font");

    let mut x_offset = 0.0;
    let mut y_offset = 0.0;
    let mut zoom = 1.0;

    let mut is_mouse_down = false;
    let mut mouse_x = 0;
    let mut mouse_y = 0;
    let mut local_mouse_x = 0.0;
    let mut local_mouse_y = 0.0;

    let tex_items = graphics::Texture::from_file("../visualizer/items.png").unwrap();
    let tex_item_width = (tex_items.size().x / 24) as i32;

    let mut x_sidebar = window.size().x - 320;

    let mut sfegui = SfEgui::new(&window);

    let mut show_load_preset_modal = false;
    let mut modal_load_preset_path = "".to_string();
    let mut error_modal_message: Option<String> = None;

    while window.is_open() {
        while let Some(ev) = window.poll_event() {
            sfegui.add_event(&ev);

            match ev {
                Event::Closed => { window.close(); }
                Event::MouseButtonPressed { button, .. } => {
                    if button == mouse::Button::Left {
                        is_mouse_down = true;
                    } else if button == mouse::Button::Middle {
                        zoom = 1.0;
                    }
                },
                Event::MouseButtonReleased { button, .. } => {
                    if button == mouse::Button::Left {
                        is_mouse_down = false;
                    }
                },
                Event::MouseWheelScrolled { wheel: _, delta, .. } => {
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

                    local_mouse_x = (mouse_x as f32 - x_offset) / zoom;
                    local_mouse_y = (mouse_y as f32 - y_offset) / zoom;
                },
                Event::Resized { width, height } => {
                    window.set_view(&graphics::View::from_rect(graphics::Rect::new(0.0, 0.0, width as f32, height as f32)).unwrap());
                    x_sidebar = width - 320;
                },
                _ => {}
            }
        }

        window.clear(Color::rgb(0x1F, 0x1F, 0x1F));

        let mut states = graphics::RenderStates::default();
        states.transform.translate(x_offset, y_offset);
        states.transform.scale(zoom, zoom);

        let mut info_overlay_opt: Option<String> = None;
        // Draw the entire map
        for i in 0..room_data.len() {
            let data = &room_data[i];
            let (x, y) = plando.map.rooms[data.room_idx];
            let room_geometry = &game_data.room_geometry[data.room_idx];

            // Draw the background color
            for (local_y, row) in room_geometry.map.iter().enumerate() {
                for (local_x, &cell) in row.iter().enumerate() {
                    if cell == 0 {
                        continue;
                    }
                    let cell_x = (local_x + x) * 8;
                    let cell_y = (local_y + y) * 8;
                    let color_value = if room_geometry.heated { 2 } else { 1 };
                    let cell_color = get_explored_color(color_value, plando.map.area[data.room_idx]);
                    let mut bg_rect = graphics::RectangleShape::with_size(Vector2f::new(8.0, 8.0));
                    bg_rect.set_position(Vector2f::new(cell_x as f32, cell_y as f32));
                    bg_rect.set_fill_color(cell_color);
                    window.draw_with_renderstates(&bg_rect, &states);

                    // Set up an info overlay we'll draw later, so it'll be on top
                    if info_overlay_opt.is_none() && graphics::FloatRect::new(cell_x as f32, cell_y as f32, 8.0, 8.0).contains2(local_mouse_x, local_mouse_y) {
                        info_overlay_opt = Some(data.room_name.to_string());
                    }
                }
            }

            // Draw the room outlines
            let mut room_sprite = graphics::Sprite::with_texture_and_rect(&atlas_tex,
                graphics::IntRect::new(8 * (data.atlas_x_offset as i32), 8 * (data.atlas_y_offset as i32),
                    8 * data.tile_width as i32, 8 * data.tile_height as i32));
            room_sprite.set_position(Vector2f::new(8.0 * x as f32, 8.0 * y as f32));
            window.draw_with_renderstates(&room_sprite, &states);

            // Draw items
            let mut found_double_item = false;
            for item in &room_geometry.items {
                let mut spr_item = graphics::Sprite::with_texture_and_rect(&tex_items,
                    graphics::IntRect::new(tex_item_width * 23, 0, tex_item_width, tex_item_width));
                spr_item.set_origin(Vector2f::new(tex_item_width as f32 / 2.0, tex_item_width as f32 / 2.0));
                let mut item_x_offset = 4;
                if let Some(double_item) = data.double_item {
                    if double_item.0 == item.x as u32 && double_item.1 == item.y as u32 {
                        item_x_offset = if found_double_item { 6 } else { 2 };
                        found_double_item = true;
                    }
                }
                spr_item.set_position(Vector2f::new((8 * (item.x + x) + item_x_offset) as f32, (8 * (item.y + y) + 4) as f32));
                spr_item.set_scale(6.0 / tex_item_width as f32 );

                window.draw_with_renderstates(&spr_item, &states);
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

        // Draw GUI
        // Draw Item/Door Select Sidebar
        let mut sidebar_rect = graphics::RectangleShape::new();
        sidebar_rect.set_position((x_sidebar as f32, 0.0));
        sidebar_rect.set_size((320.0, window.size().y as f32));
        sidebar_rect.set_fill_color(graphics::Color::rgb(0x0F, 0x0F, 0x0F));
        window.draw(&sidebar_rect);

        // Draw Menu Bar
        let gui = sfegui.run(&mut window, |_rt, ctx| {
            egui::TopBottomPanel::top("menu_file_main").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Save Seed").clicked() {
                            
                        }
                        if ui.button("Load Seed").clicked() {

                        }
                        if ui.button("Load Settings Preset").clicked() {
                            show_load_preset_modal = true;
                        }
                        if ui.button("Create ROM").clicked() {
                            
                        }
                    });
                    ui.menu_button("Map", |ui| {
                        if ui.button("Reroll Map (Vanilla)").clicked() {
                            plando.map = roll_map(&plando.maps_vanilla, &game_data).unwrap();
                            ui.close_menu();
                        }
                        if ui.button("Reroll Map (Standard)").clicked() {
                            plando.map = roll_map(&plando.maps_standard, &game_data).unwrap();
                            ui.close_menu();
                        }
                        if ui.button("Reroll Map (Wild)").clicked() {
                            plando.map = roll_map(&plando.maps_wild, &game_data).unwrap();
                        }
                    });
                });
            });

            if show_load_preset_modal {
                let modal = egui::Modal::new(Id::new("modal_load_preset")).show(ctx, |ui| {
                    ui.heading("Load Settings Preset");
                    ui.label("Filepath:");
                    ui.text_edit_singleline(&mut modal_load_preset_path);
                    ui.separator();
                    egui_sfml::egui::Sides::new().show(ui, |_ui| {}, |ui| {
                        if ui.button("Load").clicked() {
                            let preset_res = load_preset(Path::new(&modal_load_preset_path));
                            if preset_res.is_ok() {
                                randomizer_settings = preset_res.unwrap();
                                show_load_preset_modal = false;
                            } else {
                                error_modal_message = Some("Could not open supplied preset".to_string());
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            show_load_preset_modal = false;
                        }
                    });
                });
                if modal.should_close() {
                    show_load_preset_modal = false;
                }
            }

            if let Some(err_msg) = error_modal_message.clone() {
                let modal = egui::Modal::new(Id::new("modal_error")).show(ctx, |ui| {
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
        sfegui.draw(gui, &mut window, None);

        window.display();
    }
}
