use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{Context, TextureHandle, TextureOptions};

use super::extract::extract_file_icon;

const MAX_ENTRIES: usize = 512;

pub struct IconCache {
    textures: HashMap<PathBuf, TextureHandle>,
    order: Vec<PathBuf>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn file_icon(
        &mut self,
        ctx: &Context,
        path: &Path,
        size: u32,
    ) -> Option<&TextureHandle> {
        if path.as_os_str().is_empty() {
            return None;
        }
        let key = path.to_path_buf();
        if !self.textures.contains_key(&key) {
            let image = extract_file_icon(path, size)?;
            let name = format!("file_icon_{}", texture_key(&key));
            let handle = ctx.load_texture(name, image, TextureOptions::LINEAR);
            self.order.retain(|p| p != &key);
            self.order.push(key.clone());
            self.textures.insert(key.clone(), handle);
            while self.order.len() > MAX_ENTRIES {
                if let Some(old) = self.order.first().cloned() {
                    self.order.remove(0);
                    self.textures.remove(&old);
                }
            }
        }
        self.textures.get(&key)
    }
}

fn texture_key(path: &Path) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in path.to_string_lossy().bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

impl Default for IconCache {
    fn default() -> Self {
        Self::new()
    }
}
