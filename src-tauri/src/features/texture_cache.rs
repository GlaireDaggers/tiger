use image::DynamicImage;
use log::error;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{collections::HashSet, time::Duration};

use crate::utils::file_watcher::FileWatcher;
use crate::utils::handle;
use crate::utils::texture_list::TextureList;
use crate::TigerApp;

pub type Handle = handle::Handle<HashMap<PathBuf, DynamicImage>>;

impl Handle {
    pub fn init<A: TigerApp + Send + Clone + 'static>(&self, tauri_app: A, period: Duration) {
        let (mut file_watcher, events_receiver) = FileWatcher::new({
            let tauri_app = tauri_app.clone();
            move || {
                let app_state = tauri_app.app_state();
                let app = app_state.0.lock();
                app.list_textures()
            }
        });

        std::thread::spawn(move || loop {
            file_watcher.update_watched_files();
            std::thread::sleep(period);
        });

        std::thread::spawn({
            let texture_cache = self.clone();
            move || loop {
                if let Ok(Ok(events)) = events_receiver.recv() {
                    remove(
                        &events.iter().map(|e| e.path.to_owned()).collect(),
                        &texture_cache,
                    )
                }
            }
        });

        std::thread::spawn({
            let texture_cache = self.clone();
            move || loop {
                let current_entries: HashSet<PathBuf> = {
                    let cache = texture_cache.lock();
                    cache.keys().cloned().collect()
                };
                let desired_entries = {
                    let app_state = tauri_app.app_state();
                    let app = app_state.0.lock();
                    app.list_textures()
                };
                let missing_entries = desired_entries
                    .iter()
                    .filter(|p| !current_entries.contains(*p))
                    .collect::<HashSet<_>>();
                let extraneous_entries = current_entries
                    .iter()
                    .filter(|p| !desired_entries.contains(*p))
                    .collect::<HashSet<_>>();
                remove(&extraneous_entries, &texture_cache);
                add(&missing_entries, &texture_cache);
                std::thread::sleep(period);
            }
        });
    }
}

fn remove<P: AsRef<Path>>(textures: &HashSet<P>, texture_cache: &Handle) {
    let mut cache = texture_cache.lock();
    for path in textures {
        cache.remove(path.as_ref());
    }
}

fn add<P: AsRef<Path>>(textures: &HashSet<P>, texture_cache: &Handle) {
    {
        let mut cache = texture_cache.lock();
        for texture in textures {
            cache.remove(texture.as_ref());
        }
    }

    let mut new_textures = HashMap::<PathBuf, DynamicImage>::new();
    for path in textures {
        match image::open(path) {
            Ok(i) => {
                new_textures.insert(path.as_ref().to_owned(), i);
            }
            Err(e) => {
                error!(
                    "Error while preloading `{0}`: {e}",
                    path.as_ref().to_string_lossy()
                );
            }
        };
    }

    {
        let mut cache = texture_cache.lock();
        for (path, texture) in new_textures {
            cache.insert(path.to_owned(), texture);
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{mock::TigerAppMock, TigerApp};

    #[tokio::test]
    async fn adds_and_removes_textures() {
        let dir = std::env::current_dir().unwrap();
        let frame_path = dir.join("test-data/samurai-dead-all.png");

        let app = TigerAppMock::new();
        app.open_documents(vec!["test-data/samurai.tiger".into()])
            .await;

        app.wait_for_periodic_scans();
        assert!(app.texture_cache().lock().contains_key(&frame_path));

        app.delete_frame(frame_path.clone());

        app.wait_for_periodic_scans();
        assert!(!app.texture_cache().lock().contains_key(&frame_path));
    }
}
