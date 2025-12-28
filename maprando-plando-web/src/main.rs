use std::{collections::HashMap, path::Path, sync::Arc};

use actix_multipart::form::{self, MultipartForm, MultipartFormConfig, bytes::Bytes, text::Text};
use actix_web::{App, HttpResponse, HttpServer, Responder, Scope, error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound}, get, http::header::{self, ContentDisposition, DispositionParam, DispositionType}, middleware::Logger, post, web};
use anyhow::Context;
use askama::Template;
use log::info;
use maprando::{customize::{ControllerButton, ControllerConfig, CustomizeSettings, DoorTheme, FlashingSetting, MusicSettings, PaletteTheme, ShakingSetting, TileTheme, mosaic::MosaicTheme, parse_controller_button, samus_sprite::SamusSpriteCategory}, difficulty::{get_full_global, get_link_difficulty_length}, patch::Rom, preset::PresetData, randomize::Randomization, settings::{DoorLocksSize, ETankRefill, Fanfares, ItemMarkers, MotherBrainFight, ObjectiveSetting, RandomizerSettings, get_objective_groups}, spoiler_map};
use maprando_game::GameData;
use maprando_plando_backend::{seed_data::SeedData, Plando};
use serde::{Deserialize, Serialize};

use crate::file_storage::{FileStorage, Seed, SeedFile};

mod file_storage;
mod utils;

const VISUALIZER_PATH: &'static str = "./data/visualizer";

#[derive(Template)]
#[template(path = "upload.html")]
struct TemplateHome {

}

#[get("/")]
async fn home() -> impl Responder {
    let template = TemplateHome {};
    HttpResponse::Ok().body(template.render().unwrap())
}

#[derive(Debug, MultipartForm)]
struct UploadForm {
    name: Text<String>,
    desc: Text<String>,
    allow_spoiler: Option<Text<String>>,
    allow_download: Option<Text<String>>,
    #[multipart(limit = "256KB")]
    file: form::bytes::Bytes
}

#[derive(Serialize, Deserialize)]
struct FileRandomization {
    name: String,
    description: Option<String>,
    creator: String,
    allow_spoiler: bool,
    allow_download: bool,
    randomization: Randomization,
    settings: RandomizerSettings,
    logical: bool
}

#[post("/upload-seed")]
async fn upload_seed(data: web::Data<AppData>, MultipartForm(form): MultipartForm<UploadForm>) -> Result<impl Responder, actix_web::Error> {
    if form.name.len() > 32 {
        return Err(ErrorBadRequest("Plando name too long. Maximum of 32 characters."));
    }
    if form.desc.len() > 600 {
        return Err(ErrorBadRequest("Description too long. Maximum of 300 characters."));
    }

    info!("Received seed: {} ({} bytes)", form.name.0, form.file.data.len());

    let mut seed_id;
    loop {
        seed_id = utils::generate_seed_id(8);
        info!("Generated seed_id: {seed_id}");
        if data.file_storage.get_file(format!("{seed_id}/metadata.json")).await.is_err() {
            break;
        }
    }

    info!("Parsing seed data");
    let seed_data = SeedData::from_bytes(form.file.data.to_vec(), &data.game_data, &data.preset_data).map_err(
        |err| ErrorBadRequest(format!("Could not parse file: {err}"))
    )?;
    info!("Seed data parsed, creator: {}", seed_data.creator_name);

    info!("Constructing plando instance");
    let mut plando = Plando::new(data.game_data.clone(), seed_data.map.clone(), &data.preset_data).map_err(
        |err| ErrorInternalServerError(format!("Failed to construct plando instance: {err}"))
    )?;
    info!("Loading seed into plando instance");
    seed_data.clone().load_into_plando(&mut plando).map_err(
        |err| ErrorBadRequest(format!("Failed to pass parsed plando file into plando instance: {err}"))
    )?;

    info!("Updating Spoiler Log");
    plando.update_spoiler_data(true).map_err(
        |err| ErrorBadRequest(format!("Failed to update spoiler data: {err}"))
    )?.await.map_err(
        |_| ErrorInternalServerError(format!("Failed to join async handle"))
    )?.map_err(
        |err| ErrorBadRequest(format!("Failed to update spoiler data: {err}"))
    )?;

    info!("Retrieving Spoiler Log");
    let mutex = plando.get_randomization();
    let (r, s) = mutex.as_ref().ok_or_else(
        || ErrorInternalServerError("Failed to retrieve randomization instance and spoiler log")
    )?;

    let mb_flag_str = &data.game_data.flag_isv.keys[data.game_data.mother_brain_defeated_flag_id];
    let mb_clearable = s.details.iter().any(|summary| {
        summary.flags.iter().any(|flag| flag.flag == *mb_flag_str)
    });
    let logically_clearable = mb_clearable && plando.spoiler_overrides.is_empty();

    let r_json = serde_json::to_value(r)?;
    let mut s_json = serde_json::to_value(s)?;
    let s_json_map = s_json.as_object_mut().unwrap();
    s_json_map.retain(|k, _| k != "forward_traversal" && k != "reverse_traversal");
    s_json_map.insert("spoiler_overrides".to_string(), serde_json::to_value(&plando.spoiler_overrides)?);

    let desc = if form.desc.0.is_empty() {
        None
    } else {
        Some(form.desc.0)
    };

    let allow_spoiler = form.allow_spoiler.is_some_and(|x| x.0 == "on");
    let allow_download = form.allow_download.is_some_and(|x| x.0 == "on");

    let metadata = serde_json::json!({
        "name": form.name.0,
        "description": desc,
        "creator": plando.creator_name,
        "allow_spoiler": allow_spoiler,
        "allow_download": allow_download,
        "settings": plando.randomizer_settings,
        "randomization": r_json,
        "logical": logically_clearable
    }).to_string();

    let seed_data_str = serde_json::to_string_pretty(&seed_data.to_json().map_err(
        |_| ErrorInternalServerError(format!("Failed to parse seed data back into JSON"))
    )?).map_err(
        |_| ErrorInternalServerError(format!("Failed to parse seed data into a JSON string"))
    )?;

    let mut seed = Seed {
        seed_id: seed_id.clone(),
        files: Vec::new()
    };

    seed.files.push(SeedFile {
        name: "randomization.json".to_string(),
        data: metadata.as_bytes().to_vec()
    });

    // Spoiler Data
    let prefix = if allow_spoiler { "public/" } else { "private/" };
    let mut door_settings = plando.randomizer_settings.clone();
    door_settings.other_settings.door_locks_size = DoorLocksSize::Large;
    let spoiler_map = spoiler_map::get_spoiler_map(r, &data.game_data, &door_settings, false).unwrap();
    door_settings.other_settings.door_locks_size = DoorLocksSize::Small;
    let spoiler_map_small = spoiler_map::get_spoiler_map(r, &data.game_data, &door_settings, false).unwrap();

    seed.files.push(SeedFile {
        name: format!("{prefix}spoiler.json"),
        data: s_json.to_string().as_bytes().to_vec()
    });
    seed.files.push(SeedFile {
        name: format!("{prefix}map-explored.png"),
        data: spoiler_map.explored
    });
    seed.files.push(SeedFile {
        name: format!("{prefix}map-outline.png"),
        data: spoiler_map.outline
    });
    seed.files.push(SeedFile {
        name: format!("{prefix}map-explored-small.png"),
        data: spoiler_map_small.explored
    });

    // Plando file
    seed.files.push(SeedFile {
        name: if allow_download { "public/plando.json" } else { "private/plando.json" }.to_string(),
        data: seed_data_str.as_bytes().to_vec()
    });

    info!("Storing seed files");
    data.file_storage.put_seed(seed).await.map_err(
        |_| ErrorInternalServerError("Failed storing seed files")
    )?;

    info!("Seed stored successfully");
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "seed_id": seed_id
    })))
}

#[get("/{seed_id}/data/{filename:.*}")]
async fn get_seed_file(data: web::Data<AppData>, path: web::Path<(String, String)>) -> Result<impl Responder, actix_web::Error> {
    let seed_id = &path.0;
    let filename = &path.1;

    info!("get_seed_file: {seed_id}/{filename}");

    let res = if let Some(filepath) = filename.strip_prefix("visualizer/") {
        let path = Path::new(VISUALIZER_PATH).join(filepath);
        std::fs::read(&path).map_err(anyhow::Error::from).with_context(|| format!("Error reading static file: {}", path.display()))
    } else {
        let path = seed_id.clone() + "/public/" + filename;
        data.file_storage.get_file(path).await
    };

    let data = res.map_err(
        |_| ErrorNotFound("Seed file not found")
    )?;

    let ext = Path::new(filename).extension().map(|x| x.to_str().unwrap()).unwrap_or("bin");
    let mime = actix_files::file_extension_to_mime(ext);

    Ok(HttpResponse::Ok().content_type(mime).body(data))
}

#[derive(Template)]
#[template(path = "seed.html")]
struct SeedTemplate<'a> {
    name: String,
    description: Option<String>,
    creator: String,
    diff_str: String,
    qol_str: String,
    obj_str: String,
    logical: bool,
    allow_spoiler: bool,
    allow_download: bool,
    settings: RandomizerSettings,
    enabled_tech: Vec<i32>,
    enabled_notables: Vec<(usize, usize)>,
    preset_data: &'a PresetData,
    samus_sprite_categories: Vec<SamusSpriteCategory>,
    mosaic_themes: Vec<MosaicTheme>,

    item_markers: String,
    mother_brain_fight: String,
    fanfares: String,
    etank_refill: String,
    objective_names: Vec<String>
}

impl<'a> SeedTemplate<'a> {
    fn percent_enabled(&self, diff_name: &str) -> usize {
        let tech_settings = &self.settings.skill_assumption_settings.tech_settings;
        let notable_settings = &self.settings.skill_assumption_settings.notable_settings;
        let all_techs = &self.preset_data.tech_by_difficulty[diff_name];
        let all_notables = &self.preset_data.notables_by_difficulty[diff_name];

        let tech_filtered = tech_settings.iter().filter(
            |tech| tech.enabled && all_techs.contains(&tech.id)
        ).count();
        let notables_filtered = notable_settings.iter().filter(
            |notable| notable.enabled && all_notables.contains(&(notable.room_id, notable.notable_id))
        ).count();

        let enabled = tech_filtered + notables_filtered;
        let total = all_techs.len() + all_notables.len();
        let ratio = enabled as f32 / total as f32;
        let percent = (ratio * 100.0).floor() as usize;
        percent
    }
}

#[get("/{seed_id}")]
async fn get_seed_redirect(seed_id: web::Path<String>) -> impl Responder {
    HttpResponse::Found()
        .insert_header((header::LOCATION, format!("{}/", seed_id)))
        .finish()
}

#[get("/{seed_id}/")]
async fn get_seed(data: web::Data<AppData>, seed_id: web::Path<String>) -> Result<impl Responder, actix_web::Error> {
    info!("Looking up seed {seed_id}");

    let seed_file_bytes = data.file_storage.get_file(format!("{seed_id}/randomization.json")).await.map_err(
        |_| ErrorNotFound("Seed not found")
    )?;
    let seed_file = String::from_utf8(seed_file_bytes).map_err(
        |_| ErrorInternalServerError("Failed to read seed file: Contains non-utf-8 characters")
    )?;
    let r_data: FileRandomization = serde_json::from_str(&seed_file).map_err(
        |_| ErrorInternalServerError("Seed file is not valid JSON")
    )?;

    let enabled_tech = r_data.settings.skill_assumption_settings.tech_settings.iter().filter_map(
        |tech| if tech.enabled { Some(tech.id) } else { None }
    ).collect();
    let enabled_notables = r_data.settings.skill_assumption_settings.notable_settings.iter().filter_map(
        |notable| if notable.enabled { Some((notable.room_id, notable.notable_id)) } else { None }
    ).collect();

    let mut diff_str =  r_data.settings.skill_assumption_settings.preset.as_ref().unwrap_or(&"custom".to_string()).to_ascii_lowercase();
    let qol_str = r_data.settings.quality_of_life_settings.preset.as_ref().unwrap_or(&"custom".to_string()).to_ascii_lowercase();
    let obj_str = r_data.settings.objective_settings.preset.as_ref().unwrap_or(&"custom".to_string()).to_ascii_lowercase();

    if diff_str == "implicit" {
        diff_str = "basic".to_string();
    }

    let item_markers = match r_data.settings.quality_of_life_settings.item_markers {
        ItemMarkers::Simple => "Simple",
        ItemMarkers::Uniques => "Uniques",
        ItemMarkers::Majors => "Majors",
        ItemMarkers::ThreeTiered => "3-Tiered",
        ItemMarkers::FourTiered => "4-Tiered"
    }.to_string();
    let mother_brain_fight = match r_data.settings.quality_of_life_settings.mother_brain_fight {
        MotherBrainFight::Vanilla => "Vanilla",
        MotherBrainFight::Short => "Short",
        MotherBrainFight::Skip => "Skip"
    }.to_string();
    let fanfares = match r_data.settings.quality_of_life_settings.fanfares {
        Fanfares::Off => "Off",
        Fanfares::Trimmed => "Trimmed",
        Fanfares::Vanilla => "Vanilla"
    }.to_string();
    let etank_refill = match r_data.settings.quality_of_life_settings.etank_refill {
        ETankRefill::Disabled => "Disabled",
        ETankRefill::Full => "Full",
        ETankRefill::Vanilla => "Vanilla"
    }.to_string();
    let objectives_map: HashMap<String, String> = get_objective_groups()
        .iter()
        .flat_map(|x| x.objectives.clone())
        .collect();
    let objective_names = r_data.settings.objective_settings.objective_options.iter().filter_map(
        |obj| if obj.setting == ObjectiveSetting::Yes {
            Some(objectives_map[&format!("{:?}", obj.objective)].clone())
        } else {
            None
        }
    ).collect();

    let template = SeedTemplate {
        name: r_data.name,
        description: r_data.description,
        creator: r_data.creator,
        diff_str,
        qol_str,
        obj_str,
        allow_spoiler: r_data.allow_spoiler,
        allow_download: r_data.allow_download,
        settings: r_data.settings,
        enabled_tech,
        enabled_notables,
        preset_data: &data.preset_data,
        logical: r_data.logical,
        samus_sprite_categories: data.samus_sprites.clone(),
        mosaic_themes: data.mosaic_themes.clone(),
        item_markers,
        mother_brain_fight,
        fanfares,
        etank_refill,
        objective_names
    };
    let render = template.render().map_err(
        |err| ErrorInternalServerError(format!("Error rendering template: {err}"))
    )?;

    Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body(render))
}

#[derive(MultipartForm)]
struct FormCustomize {
    rom: Bytes,
    samus_sprite: Text<String>,
    etank_color: Text<String>,
    item_dot_change: Text<String>,
    transition_letters: Text<bool>,
    reserve_hud_style: Text<bool>,
    room_palettes: Text<String>,
    tile_theme: Text<String>,
    door_theme: Text<String>,
    music: Text<String>,
    disable_beeping: Text<bool>,
    shaking: Text<String>,
    flashing: Text<String>,
    vanilla_screw_attack_animation: Text<bool>,
    room_names: Text<bool>,
    control_shot: Text<String>,
    control_jump: Text<String>,
    control_dash: Text<String>,
    control_item_select: Text<String>,
    control_item_cancel: Text<String>,
    control_angle_up: Text<String>,
    control_angle_down: Text<String>,
    spin_lock_left: Option<Text<String>>,
    spin_lock_right: Option<Text<String>>,
    spin_lock_up: Option<Text<String>>,
    spin_lock_down: Option<Text<String>>,
    spin_lock_x: Option<Text<String>>,
    spin_lock_y: Option<Text<String>>,
    spin_lock_a: Option<Text<String>>,
    spin_lock_b: Option<Text<String>>,
    spin_lock_l: Option<Text<String>>,
    spin_lock_r: Option<Text<String>>,
    spin_lock_select: Option<Text<String>>,
    spin_lock_start: Option<Text<String>>,
    quick_reload_left: Option<Text<String>>,
    quick_reload_right: Option<Text<String>>,
    quick_reload_up: Option<Text<String>>,
    quick_reload_down: Option<Text<String>>,
    quick_reload_x: Option<Text<String>>,
    quick_reload_y: Option<Text<String>>,
    quick_reload_a: Option<Text<String>>,
    quick_reload_b: Option<Text<String>>,
    quick_reload_l: Option<Text<String>>,
    quick_reload_r: Option<Text<String>>,
    quick_reload_select: Option<Text<String>>,
    quick_reload_start: Option<Text<String>>,
    moonwalk: Text<bool>,
}

fn get_spin_lock_buttons(req: &FormCustomize) -> Vec<ControllerButton> {
    let mut spin_lock_buttons = vec![];
    let setting_button_mapping = vec![
        (&req.spin_lock_left, ControllerButton::Left),
        (&req.spin_lock_right, ControllerButton::Right),
        (&req.spin_lock_up, ControllerButton::Up),
        (&req.spin_lock_down, ControllerButton::Down),
        (&req.spin_lock_a, ControllerButton::A),
        (&req.spin_lock_b, ControllerButton::B),
        (&req.spin_lock_x, ControllerButton::X),
        (&req.spin_lock_y, ControllerButton::Y),
        (&req.spin_lock_l, ControllerButton::L),
        (&req.spin_lock_r, ControllerButton::R),
        (&req.spin_lock_select, ControllerButton::Select),
        (&req.spin_lock_start, ControllerButton::Start),
    ];

    for (setting, button) in setting_button_mapping {
        if let Some(x) = setting
            && x.0 == "on"
        {
            spin_lock_buttons.push(button);
        }
    }
    spin_lock_buttons
}

fn get_quick_reload_buttons(req: &FormCustomize) -> Vec<ControllerButton> {
    let mut quick_reload_buttons = vec![];
    let setting_button_mapping = vec![
        (&req.quick_reload_left, ControllerButton::Left),
        (&req.quick_reload_right, ControllerButton::Right),
        (&req.quick_reload_up, ControllerButton::Up),
        (&req.quick_reload_down, ControllerButton::Down),
        (&req.quick_reload_a, ControllerButton::A),
        (&req.quick_reload_b, ControllerButton::B),
        (&req.quick_reload_x, ControllerButton::X),
        (&req.quick_reload_y, ControllerButton::Y),
        (&req.quick_reload_l, ControllerButton::L),
        (&req.quick_reload_r, ControllerButton::R),
        (&req.quick_reload_select, ControllerButton::Select),
        (&req.quick_reload_start, ControllerButton::Start),
    ];

    for (setting, button) in setting_button_mapping {
        if let Some(x) = setting
            && x.0 == "on"
        {
            quick_reload_buttons.push(button);
        }
    }
    quick_reload_buttons
}


#[post("/{seed_id}/patch")]
async fn patch_seed(data: web::Data<AppData>, seed_id: web::Path<String>, MultipartForm(form): MultipartForm<FormCustomize>) -> Result<impl Responder, actix_web::Error> {
    info!("Patching seed {}", seed_id);

    let seed_file_bytes = data.file_storage.get_file(format!("{seed_id}/randomization.json")).await.map_err(
        |_| ErrorNotFound("Seed not found")
    )?;
    let seed_file = String::from_utf8(seed_file_bytes).map_err(
        |_| ErrorInternalServerError("Failed to read seed file: Contains non-utf-8 characters")
    )?;
    let r_data: FileRandomization = serde_json::from_str(&seed_file).map_err(
        |_| ErrorInternalServerError("Seed file is not valid JSON")
    )?;

    let rom_bytes = form.rom.data.to_vec();
    let rom_vanilla = Rom::new(rom_bytes);

    let customize_settings = CustomizeSettings {
        samus_sprite: if r_data.settings.other_settings.ultra_low_qol
            && form.samus_sprite.0 == "samus_vanilla"
            && form.vanilla_screw_attack_animation.0
        {
            None
        } else {
            Some(form.samus_sprite.0.clone())
        },
        etank_color: Some((
            u8::from_str_radix(&form.etank_color.0[0..2], 16).unwrap() / 8,
            u8::from_str_radix(&form.etank_color.0[2..4], 16).unwrap() / 8,
            u8::from_str_radix(&form.etank_color.0[4..6], 16).unwrap() / 8,
        )),
        item_dot_change: match form.item_dot_change.0.as_str() {
            "Fade" => maprando::customize::ItemDotChange::Fade,
            "Disappear" => maprando::customize::ItemDotChange::Disappear,
            _ => panic!("Unexpected item_dot_change"),
        },
        transition_letters: form.transition_letters.0,
        reserve_hud_style: form.reserve_hud_style.0,
        vanilla_screw_attack_animation: form.vanilla_screw_attack_animation.0,
        room_names: form.room_names.0,
        palette_theme: if form.room_palettes.0 == "area-themed" {
            PaletteTheme::AreaThemed
        } else {
            PaletteTheme::Vanilla
        },
        tile_theme: if form.tile_theme.0 == "none" {
            TileTheme::Vanilla
        } else if form.tile_theme.0 == "scrambled" {
            TileTheme::Scrambled
        } else if form.tile_theme.0 == "area_themed" {
            TileTheme::AreaThemed
        } else {
            TileTheme::Constant(form.tile_theme.0.to_string())
        },
        door_theme: match form.door_theme.0.as_str() {
            "vanilla" => DoorTheme::Vanilla,
            "alternate" => DoorTheme::Alternate,
            _ => panic!(
                "Unexpected door_theme option: {}",
                form.door_theme.0.as_str()
            ),
        },
        music: match form.music.0.as_str() {
            "area" => MusicSettings::AreaThemed,
            "disabled" => MusicSettings::Disabled,
            _ => panic!("Unexpected music option: {}", form.music.0.as_str()),
        },
        disable_beeping: form.disable_beeping.0,
        shaking: match form.shaking.0.as_str() {
            "Vanilla" => ShakingSetting::Vanilla,
            "Reduced" => ShakingSetting::Reduced,
            "Disabled" => ShakingSetting::Disabled,
            _ => panic!("Unexpected shaking option: {}", form.shaking.0.as_str()),
        },
        flashing: match form.flashing.0.as_str() {
            "Vanilla" => FlashingSetting::Vanilla,
            "Reduced" => FlashingSetting::Reduced,
            _ => panic!("Unexpected flashing option: {}", form.flashing.0.as_str()),
        },
        controller_config: ControllerConfig {
            shot: parse_controller_button(&form.control_shot.0).unwrap(),
            jump: parse_controller_button(&form.control_jump.0).unwrap(),
            dash: parse_controller_button(&form.control_dash.0).unwrap(),
            item_select: parse_controller_button(&form.control_item_select.0).unwrap(),
            item_cancel: parse_controller_button(&form.control_item_cancel.0).unwrap(),
            angle_up: parse_controller_button(&form.control_angle_up.0).unwrap(),
            angle_down: parse_controller_button(&form.control_angle_down.0).unwrap(),
            spin_lock_buttons: get_spin_lock_buttons(&form),
            quick_reload_buttons: get_quick_reload_buttons(&form),
            moonwalk: form.moonwalk.0,
        },
    };

    let cur_dir = std::env::current_dir()?;
    let data_dir = std::path::Path::new("./data/maprando-data");
    std::env::set_current_dir(data_dir)?;
    let rom_patched = maprando::patch::make_rom(
        &rom_vanilla,
        &r_data.settings,
        &customize_settings,
        &r_data.randomization,
        &data.game_data,
        &data.samus_sprites,
        &data.mosaic_themes
    ).map_err(
        |err| ErrorInternalServerError(format!("Failed to patch ROM: {err}"))
    )?;
    std::env::set_current_dir(cur_dir)?;

    let rom_name = if r_data.name.is_empty() {
        seed_id.to_string()
    } else {
        r_data.name.to_ascii_lowercase().replace(" ", "-")
    };

    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(
                format!("maprando-plando-{}.sfc", rom_name)
            )]
        })
        .body(rom_patched.data)
    )
}

struct AppData {
    game_data: Arc<GameData>,
    preset_data: PresetData,
    samus_sprites: Vec<SamusSpriteCategory>,
    mosaic_themes: Vec<MosaicTheme>,
    file_storage: FileStorage,
    //db_pool: Pool<Postgres>
}

async fn build_app_data() -> anyhow::Result<AppData> {
    let load_path = std::path::Path::new("./data/maprando-data/");
    let mut game_data = GameData::load(load_path)?;

    let tech_path = std::path::Path::new("./data/maprando-data/data/tech_data.json");
    let notable_path = std::path::Path::new("./data/maprando-data/data/notable_data.json");
    let presets_path = std::path::Path::new("./data/maprando-data/data/presets");
    let preset_data = PresetData::load(tech_path, notable_path, presets_path, &game_data)?;
    let global = get_full_global(&game_data);
    game_data.make_links_data(&|link, game_data| {
        get_link_difficulty_length(link, game_data, &preset_data, &global)
    });

    let samus_sprite_path = std::path::Path::new("./data/MapRandoSprites/samus_sprites/manifest.json");
    let samus_sprites: Vec<SamusSpriteCategory> = serde_json::from_str(&std::fs::read_to_string(samus_sprite_path)?)?;

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
        ("Outline", "Practice Outlines"),
        ("Invisible", "Invisible")
    ].into_iter()
        .map(|(x, y)| MosaicTheme {
            name: x.to_string(),
            display_name: y.to_string(),
        }).collect();

    let file_storage_url = std::env::var("FILE_STORAGE")?;
    let file_storage = FileStorage::new(&file_storage_url);

    //let db_address = std::env::var("DATABASE_URL")?;
    //let pool = PgPool::connect(&db_address).await?;

    Ok(AppData {
        game_data: Arc::new(game_data),
        preset_data,
        samus_sprites,
        mosaic_themes,
        file_storage
    //    db_pool: pool
    })
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();
    dotenvy::dotenv()?;

    let data = web::Data::new(build_app_data().await?);

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .app_data(MultipartFormConfig::default()
                .total_limit(4_000_000)
                .memory_limit(4_000_000)
            )
            .wrap(Logger::default())
            .service(home)
            .service(upload_seed)
            .service(Scope::new("seed")
                .service(get_seed_redirect)
                .service(get_seed)
                .service(get_seed_file)
                .service(patch_seed)
            )
            .service(actix_files::Files::new("/js", "./maprando-plando-web/js"))
            .service(actix_files::Files::new("/css", "./maprando-plando-web/css"))
            .service(actix_files::Files::new("/img", "./maprando-plando-web/img"))
    })
    .workers(1)
    .bind(("127.0.0.1", 8080))?
    .run().await?;

    Ok(())
}
