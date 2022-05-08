use euclid::default::*;
use imgui::StyleVar::*;
use imgui::*;
use std::time::Duration;

use crate::sheet::*;
use crate::state::*;
use crate::streamer::{TextureCache, TextureCacheResult};
use crate::ui::spinner::*;
use crate::utils;
use crate::utils::*;

fn draw_frame<'a>(ui: &Ui<'a>, texture_cache: &TextureCache, frame: &Frame) {
    if let Some(name) = frame.get_source().file_name() {
        ui.text(&ImString::new(name.to_string_lossy()));
        let space = ui.content_region_avail().into();
        match texture_cache.get(frame.get_source()) {
            Some(TextureCacheResult::Loaded(texture)) => {
                if let Some(fill) = utils::fill(space, texture.size) {
                    let cursor_pos = Vector2D::<f32>::from(ui.cursor_pos());
                    let draw_position = cursor_pos + fill.rect.origin.to_vector();
                    ui.set_cursor_pos(draw_position.to_array());
                    Image::new(texture.id, fill.rect.size.to_array()).build(ui);
                }
            }
            Some(TextureCacheResult::Loading) => {
                draw_spinner(ui, &ui.get_window_draw_list(), space);
            }
            _ => {
                // TODO
            }
        }
    }
}

fn draw_hitbox<'a>(ui: &Ui<'a>, hitbox: &Hitbox) {
    let position = hitbox.get_position();
    let size = hitbox.get_size();
    ui.text(&ImString::new(format!("Tag: {}", hitbox.get_name())));
    ui.text(&ImString::new(format!(
        "Offset: {}, {}",
        position.x, position.y
    )));
    ui.text(&ImString::new(format!(
        "Dimensions: {} x {}",
        size.x, size.y
    )));

    let space: Vector2D<f32> = ui.content_region_avail().into();
    let padding = 0.2;

    if let Some(fill) = utils::fill(space * (1.0 - padding), size.to_f32()) {
        let cursor_screen_pos: Vector2D<f32> = ui.cursor_screen_pos().into();
        let draw_list = ui.get_window_draw_list();
        let color = [1.0, 1.0, 1.0, 1.0]; // TODO.style
        draw_list
            .add_rect(
                (cursor_screen_pos + space * padding / 2.0 + fill.rect.min().to_vector())
                    .to_array(),
                (cursor_screen_pos + space * padding / 2.0 + fill.rect.max().to_vector())
                    .to_array(),
                color,
            )
            .thickness(2.0) // TODO dpi
            .build();
    }
}

fn draw_animation<'a>(
    ui: &Ui<'a>,
    app_state: &AppState,
    texture_cache: &TextureCache,
    animation: &Animation,
) {
    ui.text(&ImString::new(animation.get_name().to_owned()));
    let space = ui.content_region_avail().into();
    match utils::get_bounding_box(animation, texture_cache) {
        Ok(mut bbox) => {
            bbox.center_on_origin();
            if let Some(fill) = utils::fill(space, bbox.rect.size.to_f32().to_vector()) {
                let duration = animation.get_duration().unwrap(); // TODO no unwrap
                let time = Duration::from_millis(
                    app_state.get_clock().as_millis() as u64 % u64::from(duration),
                ); // TODO pause on first and last frame for non looping animation?
                let (_, keyframe) = animation.get_frame_at(time).unwrap(); // TODO no unwrap
                match texture_cache.get(keyframe.get_frame()) {
                    Some(TextureCacheResult::Loaded(texture)) => {
                        let cursor_pos: Vector2D<f32> = ui.cursor_pos().into();
                        let frame_offset = keyframe.get_offset().to_f32();
                        let draw_position = cursor_pos
                            + fill.rect.origin.to_vector()
                            + (frame_offset
                                - bbox.rect.origin.to_f32().to_vector()
                                - texture.size / 2.0)
                                * fill.zoom;
                        let draw_size = texture.size * fill.zoom;
                        ui.set_cursor_pos(draw_position.to_array());
                        Image::new(texture.id, draw_size.to_array()).build(ui);
                    }
                    Some(TextureCacheResult::Loading) => {
                        draw_spinner(ui, &ui.get_window_draw_list(), space);
                    }
                    _ => {
                        // TODO
                    }
                }
            }
        }
        Err(BoundingBoxError::FrameDataNotLoaded) => {
            draw_spinner(ui, &ui.get_window_draw_list(), space)
        }
        _ => (),
    }
}

fn draw_keyframe<'a>(ui: &Ui<'a>, texture_cache: &TextureCache, keyframe: &Keyframe) {
    let frame = keyframe.get_frame();
    if let Some(name) = frame.file_name() {
        ui.text(&ImString::new(name.to_string_lossy()));
        ui.text(&ImString::new(format!(
            "Duration: {}ms",
            keyframe.get_duration()
        )));
        let space = ui.content_region_avail().into();
        match texture_cache.get(frame) {
            Some(TextureCacheResult::Loaded(texture)) => {
                if let Some(fill) = utils::fill(space, texture.size) {
                    let cursor_pos: Vector2D<f32> = ui.cursor_pos().into();
                    let draw_position = cursor_pos + fill.rect.origin.to_vector();
                    ui.set_cursor_pos(draw_position.to_array());
                    Image::new(texture.id, fill.rect.size.to_array()).build(ui);
                }
            }
            Some(TextureCacheResult::Loading) => {
                draw_spinner(ui, &ui.get_window_draw_list(), space);
            }
            _ => {
                // TODO
            }
        }
    }
}

pub fn draw<'a>(ui: &Ui<'a>, rect: &Rect<f32>, app_state: &AppState, texture_cache: &TextureCache) {
    let _style_rounding = ui.push_style_var(WindowRounding(0.0));
    let _style_border = ui.push_style_var(WindowBorderSize(0.0));
    Window::new("Selection")
        .position(rect.origin.to_array(), Condition::Always)
        .size(rect.size.to_array(), Condition::Always)
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .build(ui, || {
            if let Some(document) = app_state.get_current_document() {
                match &document.view.selection {
                    Some(Selection::Frame(paths)) => {
                        let path = &paths.last_touched_in_range;
                        if let Some(frame) = document.sheet.get_frame(path) {
                            draw_frame(ui, texture_cache, frame);
                        }
                    }
                    Some(Selection::Animation(names)) => {
                        let name = &names.last_touched_in_range;
                        if let Some(animation) = document.sheet.get_animation(name) {
                            draw_animation(ui, app_state, texture_cache, animation);
                        }
                    }
                    Some(Selection::Keyframe(indexes)) => {
                        if let Some(WorkbenchItem::Animation(name)) = &document.view.workbench_item
                        {
                            let index = indexes.last_touched_in_range;
                            if let Some(animation) = document.sheet.get_animation(name) {
                                if let Some(keyframe) = animation.get_frame(index) {
                                    draw_keyframe(ui, texture_cache, keyframe);
                                }
                            }
                        }
                    }
                    Some(Selection::Hitbox(names)) => {
                        let name = &names.last_touched_in_range;
                        if let Ok((_, keyframe)) = document.get_workbench_keyframe() {
                            if let Some(hitbox) = keyframe.get_hitbox(name) {
                                draw_hitbox(ui, hitbox);
                            }
                        }
                    }
                    None => (),
                }
            }
        });
}
