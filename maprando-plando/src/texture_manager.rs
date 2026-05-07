use std::{any::Any, hash::{DefaultHasher, Hash, Hasher}};

use anyhow::{anyhow, bail, Result};
use egui::TextureId;
use hashbrown::HashMap;
use sfml::{cpp::FBox, graphics::Texture};

use crate::egui_sfml::UserTexSource;

pub struct TextureManager {
    tex_map: HashMap<u64, FBox<Texture>>
}

impl TextureManager {
    pub fn new() -> Self {
        TextureManager { tex_map: HashMap::new() }
    }

    pub fn add_texture<Id: Hash + std::fmt::Debug + 'static>(&mut self, id: Id, tex: FBox<Texture>) -> Result<()> {
        let err_msg = format!("Texture map already contains key {id:?}");
        let hash = get_hash(id);
        if self.tex_map.contains_key(&hash) {
            bail!(err_msg);
        }
        self.tex_map.insert(hash, tex);
        Ok(())
    }

    pub fn exists<Id: Hash + 'static>(&self, id: Id) -> bool {
        let hash = get_hash(id);
        self.tex_map.contains_key(&hash)
    }

    pub fn get_texture_const<Id: Hash + std::fmt::Debug + 'static>(&self, id: Id) -> Result<&FBox<Texture>> {
        let err_msg = format!("No texture with id {id:?}");
        let hash = get_hash(id);
        self.tex_map.get(&hash).ok_or_else(|| anyhow!(err_msg))
    }

    pub fn get_image_source<Id: Hash + 'static>(&mut self, id: Id) -> (TextureId, egui::Vec2) {
        let hash = get_hash(id);
        let tex = &self.tex_map[&hash];
        let x = tex.size().x as f32;
        let y = tex.size().y as f32;
        (TextureId::User(hash), egui::Vec2::new(x, y))
    }
}

impl UserTexSource for TextureManager {
    fn get_texture(&mut self, id: u64) -> (f32, f32, &Texture) {
        let tex = &self.tex_map[&id];
        let size = tex.size();
        (size.x as f32, size.y as f32, tex)
    }
}

fn get_hash<Id: Hash + 'static>(id: Id) -> u64 {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    id.type_id().hash(&mut hasher);
    hasher.finish()
}