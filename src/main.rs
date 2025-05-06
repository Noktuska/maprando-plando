use {
    anyhow::{bail, Context, Result}, egui_sfml::{egui::{self, Color32, Id, Sense, TextureId, Vec2}, SfEgui, UserTexSource}, hashbrown::HashMap, maprando::{
        patch::Rom, preset::PresetData, randomize::Randomizer, settings::{DoorsMode, SaveAnimals}
    }, maprando_game::{BeamType, DoorType, GameData, Item, MapTileEdge, MapTileInterior, MapTileSpecialType}, plando::{DoubleItemPlacement, MapRepositoryType, Placeable, Plando, TileInfo}, rand::Rng, sfml::{
        cpp::FBox, graphics::{
            self, Color, IntRect, RenderTarget, RenderWindow, Shape, Transformable
        }, system::Vector2f, window::{
            mouse, Event, Key, Style
        }
    }, std::{cmp::max, path::Path, u32}
};

mod plando;

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

    let (room_data, _): (Vec<_>, Vec<_>) = image_mappings.into_iter().unzip();

    Ok((atlas, room_data))
}

fn generate_door_sprites() -> Result<FBox<graphics::Image>> {
    let mut img_doors = graphics::Image::new_solid(3 * 8, 8, Color::TRANSPARENT).unwrap();
    for x in 0..8 {
        let door_color_index = match x {
            0 | 5 => 7,
            1 | 7 => 14,
            2 | 6 => 6,
            3 => 15,
            4 => 8,
            _ => 15
        };
        
        img_doors.set_pixel(3 * x + 2, 3, get_explored_color(12, 0))?;
        img_doors.set_pixel(3 * x + 2, 4, get_explored_color(12, 0))?;
        
        img_doors.set_pixel(3 * x, 3, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x + 1, 3, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x, 4, get_explored_color(door_color_index, 0))?;
        img_doors.set_pixel(3 * x + 1, 4, get_explored_color(door_color_index, 0))?;
        
        if x < 3 {
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

fn put_placeable(plando: &mut Plando, tile_info: &TileInfo, placeable: Placeable, right: bool, direction: String) -> Result<()> {
    let placed_item_count = plando.placed_item_count[placeable as usize] as usize;
    let max_item_count = plando.get_max_placeable_count(placeable);

    if placeable == Placeable::Helm {
        for i in 0..plando.game_data.start_locations.len() {
            let start_pos = plando.game_data.start_locations[i].clone();
            let tile_x = (start_pos.x / 16.0).floor() as usize;
            let tile_y = (start_pos.y / 16.0).floor() as usize;
            if tile_info.room_id == start_pos.room_id && tile_info.tile_x == tile_x && tile_info.tile_y == tile_y {
                return plando.place_start_location(start_pos);
            }
        }
        return Ok(());
    } else if let Some(item) = placeable.to_item() {
        if max_item_count.is_none() || placed_item_count < max_item_count.unwrap() {
            return plando.place_item(tile_info, item, right);
        }
        return Ok(());
    }
    let door = match placeable {
        Placeable::DoorMissile => Some(DoorType::Red),
        Placeable::DoorSuper => Some(DoorType::Green),
        Placeable::DoorPowerBomb => Some(DoorType::Yellow),
        Placeable::DoorCharge => Some(DoorType::Beam(BeamType::Charge)),
        Placeable::DoorSpazer => Some(DoorType::Beam(BeamType::Spazer)),
        Placeable::DoorWave => Some(DoorType::Beam(BeamType::Wave)),
        Placeable::DoorIce => Some(DoorType::Beam(BeamType::Ice)),
        Placeable::DoorPlasma => Some(DoorType::Beam(BeamType::Plasma)),
        _ => None
    };
    plando.place_door(tile_info, door, direction, false)
}

fn patch_rom(plando: &mut Plando, rom: &Rom) -> Result<Rom> {
    let preset_data = load_preset_data(&plando.game_data)?;
    let difficulty_tiers = maprando::randomize::get_difficulty_tiers(
        &plando.randomizer_settings, 
        &preset_data.difficulty_tiers, 
        &plando.game_data, 
        &preset_data.tech_by_difficulty["Implicit"],
        &preset_data.notables_by_difficulty["Implicit"]
    );

    let save_animals = if plando.randomizer_settings.save_animals == SaveAnimals::Random {
        if plando.rng.gen_bool(0.5) { SaveAnimals::Yes } else { SaveAnimals::No }
    } else { plando.randomizer_settings.save_animals };

    let toilet_intersections = Randomizer::get_toilet_intersections(&plando.map, &plando.game_data);

    let locked_door_data = plando.get_locked_door_data();

    Err(anyhow::anyhow!("todo"))
    /*let randomization = Randomization {
        settings: plando.randomizer_settings.clone(),
        difficulty: difficulty_tiers[0].clone(),
        objectives: plando.objectives.clone(),
        save_animals,
        map: plando.map.clone(),
        toilet_intersections,
        locked_door_data,
        item_placement: Vec::new(),
        start_location: plando.start_location.clone(),
        spoiler_log
    };
    
    maprando::patch::make_rom(&rom, &randomization, &plando.game_data)*/
}


fn main() {

    let mut plando = Plando::new();

    let rom_path = Path::new("C:/Users/Loptr/Desktop/Super Metroid/Original ROM/Super Metroid (JU) [!].smc");
    let rom_vanilla = load_vanilla_rom(rom_path).unwrap();

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
    let (atlas_img, room_data) = load_room_sprites(&plando.game_data).unwrap();
    let atlas_tex = graphics::Texture::from_image(&atlas_img, IntRect::default()).unwrap();

    let mut window = RenderWindow::new((1080, 720), "Maprando Plando", Style::DEFAULT, &Default::default()).expect("Could not create Window");
    window.set_vertical_sync_enabled(true);

    let font_default = graphics::Font::from_file("./res/segoeui.ttf").expect("Could not load default font");

    let mut x_offset = 0.0;
    let mut y_offset = 0.0;
    let mut zoom = 1.0;

    let mut is_mouse_dragged = false;
    let mut is_mouse_down = false;
    let mut mouse_x = 0;
    let mut mouse_y = 0;

    let img_items = graphics::Image::from_file("../visualizer/items.png").unwrap();
    let tex_items = graphics::Texture::from_image(&img_items, IntRect::default()).unwrap();
    let tex_item_width = (tex_items.size().x / 24) as i32;

    let img_doors = generate_door_sprites().unwrap();
    let img_door_width = (img_doors.size().x / 8) as i32;

    let tex_helm = graphics::Texture::from_file("../visualizer/helm.png").unwrap();
    let mut user_tex_source = ImplUserTexSource::new();
    
    user_tex_source.add_texture(Placeable::Helm as u64, tex_helm);

    // Add item textures to egui
    for i in 0..22 {
        let source_rect = IntRect::new(i * tex_item_width, 0, tex_item_width, img_items.size().y as i32);
        let tex = graphics::Texture::from_image(&img_items, source_rect).unwrap();
        user_tex_source.add_texture(Placeable::ETank as u64 + i as u64, tex);
    }
    // Add Door textures to egui
    for i in 0..8 {
        let source_rect = IntRect::new(i * img_door_width, 0, img_door_width, img_doors.size().y as i32);
        let tex = graphics::Texture::from_image(&img_doors, source_rect).unwrap();
        user_tex_source.add_texture(Placeable::DoorMissile as u64 + i as u64, tex);
    }

    let mut sidebar_width = 0.0;
    let sidebar_height = 32.0;

    let mut sfegui = SfEgui::new(&window);

    let mut show_load_preset_modal = false;
    let mut modal_load_preset_path = "".to_string();
    let mut error_modal_message: Option<String> = None;

    let mut sidebar_selection: Option<Placeable> = None;
    let mut spoiler_step = 1;
    let mut spoiler_item: Option<usize> = None;
    let mut spoiler_flag: Option<usize> = None;

    while window.is_open() {
        let local_mouse_x = (mouse_x as f32 - x_offset) / zoom;
        let local_mouse_y = (mouse_y as f32 - y_offset) / zoom;
        let tile_x = (local_mouse_x / 8.0).floor().max(0.0) as usize;
        let tile_y = (local_mouse_y / 8.0).floor().max(0.0) as usize;
        let tile_hovered_opt = plando.get_tile_at(tile_x, tile_y);

        while let Some(ev) = window.poll_event() {
            sfegui.add_event(&ev);

            match ev {
                Event::Closed => { window.close(); }
                Event::MouseButtonPressed { button, x, .. } => {
                    if x < window.size().x as i32 - sidebar_width as i32 {
                        if button == mouse::Button::Left {
                            is_mouse_down = true;
                        } else if button == mouse::Button::Middle {
                            zoom = 1.0;
                        }
                    }
                },
                Event::MouseButtonReleased { button, .. } => {
                    if button == mouse::Button::Left {
                        is_mouse_down = false;
                    }

                    if !is_mouse_dragged && mouse_x < window.size().x as i32 - sidebar_width as i32 {
                        if let Some(tile_info) = &tile_hovered_opt {
                            let right = (local_mouse_x / 8.0).fract() > 0.5;
                            let tr = (local_mouse_x / 8.0).fract() > (local_mouse_y / 8.0).fract();
                            let br = (local_mouse_x / 8.0).fract() > 1.0 - (local_mouse_y / 8.0).fract();
                            let direction = (if tr && br { "right" } else if tr && !br { "up" } else if !tr && br { "down" } else { "left" }).to_string();
                            if button == mouse::Button::Right && sidebar_selection.is_some() {
                                let selection = sidebar_selection.unwrap();
                                let mut is_ok = false;
                                if selection.to_item().is_some() {
                                    is_ok = plando.place_item(tile_info, Item::Nothing, right).is_ok();
                                } else if selection != Placeable::Helm {
                                    is_ok = plando.place_door(tile_info, None, direction, false).is_ok();
                                }
                                if !is_ok {
                                    sidebar_selection = None;
                                }
                            } else if button == mouse::Button::Left {
                                if let Some(selection) = sidebar_selection {
                                    let res = put_placeable(&mut plando, tile_info, selection, right, direction);
                                    if res.is_err() {
                                        error_modal_message = Some(res.unwrap_err().to_string());
                                    }
                                } else {
                                    // TODO: Select Item/Flag to reveal spoiler route
                                }
                            }
                        }
                    }
                    is_mouse_dragged = false;
                },
                Event::MouseWheelScrolled { wheel: _, delta, x, .. } => {
                    if x < window.size().x as i32 - sidebar_width as i32 {
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
                        is_mouse_dragged = true;
                    }
                    mouse_x = x;
                    mouse_y = y;
                },
                Event::Resized { width, height } => {
                    window.set_view(&graphics::View::from_rect(graphics::Rect::new(0.0, 0.0, width as f32, height as f32)).unwrap());
                },
                Event::KeyPressed { code, .. } => {
                    if code == Key::F7 {
                        plando.update_spoiler_data();
                    }
                }
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
            let room_geometry = &plando.game_data.room_geometry[data.room_idx];

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
                    window.draw_with_renderstates(&sprite_tile, &states);

                    let (tex_helm_w, _, tex_helm) = user_tex_source.get_texture(Placeable::Helm as u64);
                    let mut sprite_helm = graphics::Sprite::with_texture(tex_helm);
                    sprite_helm.set_scale(8.0 / tex_helm_w);

                    // Draw all possible starting positions
                    if sidebar_selection.is_some_and(|x| x == Placeable::Helm) {
                        sprite_helm.set_color(Color::rgba(0xAF, 0xAF, 0xAF, 0x5F));
                        for start_pos in &plando.game_data.start_locations {
                            if data.room_id != start_pos.room_id {
                                continue;
                            }
                            let start_tile_x = (start_pos.x / 16.0).floor() as usize;
                            let start_tile_y = (start_pos.y / 16.0).floor() as usize;
                            if start_tile_x == local_x && start_tile_y == local_y {
                                sprite_helm.set_position(Vector2f::new(cell_x as f32, cell_y as f32));
                                window.draw_with_renderstates(&sprite_helm, &states);
                            }
                        }
                        sprite_helm.set_color(Color::WHITE);
                    }

                    // Draw Start Position
                    if data.room_id == plando.start_location_data.start_location.room_id {
                        let start_tile_x = (plando.start_location_data.start_location.x / 16.0).floor() as usize;
                        let start_tile_y = (plando.start_location_data.start_location.y / 16.0).floor() as usize;
                        if start_tile_x == local_x && start_tile_y == local_y {
                            sprite_helm.set_position(Vector2f::new(cell_x as f32, cell_y as f32));
                            window.draw_with_renderstates(&sprite_helm, &states);
                        }
                    }
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

        // Draw items
        if sidebar_selection.is_none() || sidebar_selection.is_some_and(|x| x >= Placeable::ETank && x <= Placeable::WalljumpBoots) {
            for item_placement in &plando.item_locations {
                let (room_x, room_y) = plando.map.rooms[item_placement.room_idx];
                let item_index = match item_placement.item {
                    Item::Nothing => 23,
                    item => item as i32
                };

                let mut spr_item = graphics::Sprite::with_texture_and_rect(&tex_items,
                    IntRect::new(tex_item_width * item_index, 0, tex_item_width, tex_item_width));
                spr_item.set_origin(Vector2f::new(tex_item_width as f32 / 2.0, tex_item_width as f32 / 2.0));
                let item_x_offset = match item_placement.double_item_placement {
                    DoubleItemPlacement::Left => 2,
                    DoubleItemPlacement::Middle => 4,
                    DoubleItemPlacement::Right => 6
                };
                spr_item.set_position(Vector2f::new((8 * (item_placement.tile_x + room_x) + item_x_offset) as f32, (8 * (item_placement.tile_y + room_y) + 4) as f32));
                spr_item.set_scale(6.0 / tex_item_width as f32);
                if let Some(tile_hovered) = &tile_hovered_opt {
                    if tile_hovered.room_idx == item_placement.room_idx && tile_hovered.tile_x == item_placement.tile_x && tile_hovered.tile_y == item_placement.tile_y {
                        if item_placement.double_item_placement == DoubleItemPlacement::Middle || (item_placement.double_item_placement == DoubleItemPlacement::Left && (local_mouse_x / 8.0).fract() <= 0.5) {
                            spr_item.scale(1.2);
                        }
                    }
                }

                window.draw_with_renderstates(&spr_item, &states);
            }
        }

        // Draw flags
        

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
        /*if spoiler_item.is_some() || spoiler_flag.is_some() {
            let mut obtain_route;
            let mut return_route;
            if let Some(spoiler_idx) = spoiler_item {
                let (room_id, node_id) = plando.game_data.item_locations[spoiler_idx];
                (obtain_route, return_route) = plando.spoiler_details_vec[spoiler_step].items.iter().find_map(|x| {
                    if x.location.room_id == room_id && x.location.node_id == node_id { Some((&x.obtain_route, &x.return_route)) } else { None }
                }).unwrap();
            } else {
                let spoiler_idx = spoiler_flag.unwrap();
                let vertex_idxs = &plando.game_data.flag_vertex_ids[spoiler_idx];
                let vertex_data = &plando.game_data.vertex_isv.keys[vertex_idxs[0]];
                let (room_id, node_id) = (vertex_data.room_id, vertex_data.node_id);
                (obtain_route, return_route) = plando.spoiler_details_vec[spoiler_step].flags.iter().find_map(|x| {
                    if x.location.room_id == room_id && x.location.node_id == node_id { Some((&x.obtain_route, &x.return_route)) } else { None }
                }).unwrap();
            }

            
        }*/

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
                            patch_rom(&mut plando, &rom_vanilla).unwrap();
                        }
                    });
                    ui.menu_button("Map", |ui| {
                        if ui.button("Reroll Map (Vanilla)").clicked() {
                            plando.reroll_map(MapRepositoryType::Vanilla).unwrap();
                            ui.close_menu();
                        }
                        if ui.button("Reroll Map (Standard)").clicked() {
                            plando.reroll_map(MapRepositoryType::Standard).unwrap();
                            ui.close_menu();
                        }
                        if ui.button("Reroll Map (Wild)").clicked() {
                            plando.reroll_map(MapRepositoryType::Wild).unwrap();
                            ui.close_menu();
                        }
                    });
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
                            ui.end_row();
                        }
                    });
                });
            }).response.rect.width();

            if show_load_preset_modal {
                let modal = egui::Modal::new(Id::new("modal_load_preset")).show(ctx, |ui| {
                    ui.heading("Load Settings Preset");
                    ui.label("Filepath:");
                    ui.text_edit_singleline(&mut modal_load_preset_path);
                    ui.separator();
                    egui_sfml::egui::Sides::new().show(ui, |_ui| {}, |ui| {
                        if ui.button("Load").clicked() {
                            match plando.load_preset(Path::new(&modal_load_preset_path)) {
                                Ok(_) => show_load_preset_modal = false,
                                Err(_) => error_modal_message = Some("Could not open supplied preset".to_string())
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