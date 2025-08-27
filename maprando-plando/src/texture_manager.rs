use anyhow::{anyhow, bail, Result};
use egui::TextureId;
use hashbrown::HashMap;
use sfml::{cpp::FBox, graphics::Texture};

use crate::egui_sfml::UserTexSource;

pub struct TextureManager {
    tex_map: HashMap<usize, FBox<Texture>>
}

impl TextureManager {
    pub fn new() -> Self {
        TextureManager { tex_map: HashMap::new() }
    }

    pub fn add_texture(&mut self, id: usize, tex: FBox<Texture>) -> Result<()> {
        if self.tex_map.contains_key(&id) {
            bail!("Texture map already contains key {id}");
        }
        self.tex_map.insert(id, tex);
        Ok(())
    }

    pub fn exists<Id: Into<usize>>(&self, id: Id) -> bool {
        self.tex_map.contains_key(&id.into())
    }

    pub fn get_texture_const<Id: Into<usize>>(&self, id: Id) -> Result<&FBox<Texture>> {
        let id: usize = id.into();
        self.tex_map.get(&id).ok_or_else(|| anyhow!("No texture with id {}", id))
    }

    pub fn get_image_source(&mut self, tex_id: u64) -> (TextureId, egui::Vec2) {
        let (x, y, _tex) = self.get_texture(tex_id);
        (TextureId::User(tex_id), egui::Vec2::new(x, y))
    }
}

impl UserTexSource for TextureManager {
    fn get_texture(&mut self, id: u64) -> (f32, f32, &Texture) {
        let tex = self.get_texture_const(id as usize).unwrap();
        let size = tex.size();
        (size.x as f32, size.y as f32, tex)
    }
}