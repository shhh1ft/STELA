use egui::TextureHandle;
use std::collections::HashMap;

/// Kept so project state can drop any cached GPU images after a photo edit.
#[derive(Default)]
pub struct TextureCache {
    photos: HashMap<String, TextureHandle>,
}

impl TextureCache {
    pub fn clear(&mut self) {
        self.photos.clear();
    }
}
