use imgui::draw_list::DrawListMut;
use imgui::StyleVar::*;
use imgui::*;
use std::time::Duration;

use crate::sheet::{Animation, Keyframe};
use crate::state::*;
use crate::ui::Rect;

fn draw_timeline_ticks<'a>(ui: &Ui<'a>, commands: &mut CommandBuffer, document: &Document) {
    let zoom = document.view.get_timeline_zoom_factor();
    let h = 8.0; // TODO.dpi?
    let padding = 4.0; // TODO.dpi?

    let draw_list = ui.get_window_draw_list();
    let cursor_start = ui.cursor_screen_pos();
    let max_draw_x = cursor_start[0] + ui.content_region_avail()[0]
        - ui.window_content_region_min()[0]
        + 2.0 * ui.cursor_pos()[0]; // TODO.dpi on 2x factor?

    let mut x = cursor_start[0];
    let mut delta_t = 0;
    while x < max_draw_x {
        let (color, tick_height) = if delta_t % 100 == 0 {
            ([70.0 / 255.0, 70.0 / 255.0, 70.0 / 255.0], h) // TODO.style
        } else {
            ([20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0], h / 2.0) // TODO.style
        };

        draw_list.add_rect_filled_multicolor(
            [x, cursor_start[1]],
            [x + 1.0, cursor_start[1] + tick_height],
            color,
            color,
            color,
            color,
        );

        delta_t += 10;
        x = cursor_start[0] + delta_t as f32 * zoom;
    }

    let clicked = ui.invisible_button(
        "timeline_ticks",
        [max_draw_x - cursor_start[0], h + padding],
    );
    if ui.is_item_hovered()
        && ui.is_mouse_down(MouseButton::Left)
        && !ui.is_mouse_dragging(MouseButton::Left)
    {
        commands.begin_scrub();
    }
    if clicked || document.is_scrubbing_timeline() {
        let mouse_pos = ui.io().mouse_pos;
        let delta = mouse_pos[0] - cursor_start[0];
        let new_t = delta / zoom;
        commands.update_scrub(Duration::from_millis(std::cmp::max(0, new_t as i64) as u64));
    }

    ui.set_cursor_screen_pos([cursor_start[0], cursor_start[1] + h + padding]);
}

fn draw_insert_marker<'a>(ui: &Ui<'a>, draw_list: &DrawListMut<'_>, height: f32) {
    let position = ui.cursor_screen_pos();
    let insert_marker_size = 8.0; // TODO.dpi?
    let insert_marker_color = [249.0 / 255.0, 40.0 / 255.0, 50.0 / 255.0];
    let marker_top_left = [position[0] - insert_marker_size / 2.0, position[1]];
    let marker_bottom_right = [position[0] + insert_marker_size / 2.0, position[1] + height];
    draw_list.add_rect_filled_multicolor(
        marker_top_left,
        marker_bottom_right,
        insert_marker_color,
        insert_marker_color,
        insert_marker_color,
        insert_marker_color,
    );
}

struct FrameLocation {
    top_left: (f32, f32),
    size: (f32, f32),
}

fn get_frame_location(
    document: &Document,
    frame_starts_at: Duration,
    keyframe: &Keyframe,
) -> FrameLocation {
    let zoom = document.view.get_timeline_zoom_factor();
    let w = (keyframe.get_duration() as f32 * zoom).ceil();
    let h = 32.0; // TODO.dpi?
    let top_left = ((frame_starts_at.as_millis() as f32 * zoom).floor(), 0.0);
    FrameLocation {
        top_left,
        size: (w, h),
    }
}

fn draw_keyframe<'a>(
    ui: &Ui<'a>,
    commands: &mut CommandBuffer,
    document: &Document,
    animation: &Animation,
    keyframe_index: usize,
    keyframe: &Keyframe,
    frames_cursor_position_start: [f32; 2],
    frame_starts_at: Duration,
) {
    let keyframe_location = get_frame_location(document, frame_starts_at, keyframe);
    let zoom = document.view.get_timeline_zoom_factor();
    let outline_size = 1.0; // TODO.dpi?
    let text_padding = 4.0; // TODO.dpi?
    let max_resize_handle_size = 16.0; // TODO.dpi?
    let w = keyframe_location.size.0;
    let h = keyframe_location.size.1;
    let is_too_small_to_draw = w < 2.0 * outline_size + 1.0;

    let resize_handle_size_left = (w / 3.0).floor().min(max_resize_handle_size);
    let resize_handle_size_right = match animation.get_keyframe(keyframe_index + 1) {
        None => resize_handle_size_left,
        Some(n) => {
            let nw = (n.get_duration() as f32 * zoom).ceil();
            (nw / 3.0).floor().min(max_resize_handle_size)
        }
    };
    let resize_handle_size = resize_handle_size_left
        .min(resize_handle_size_right)
        .max(1.0);

    let is_selected = document.is_keyframe_selected(keyframe_index);
    let draw_list = ui.get_window_draw_list();
    let mut cursor_pos = ui.cursor_screen_pos();
    cursor_pos[0] += keyframe_location.top_left.0;

    let top_left = cursor_pos;
    let bottom_right = [top_left[0] + w, top_left[1] + h];

    // Draw outline
    {
        let outline_color = [25.0 / 255.0, 15.0 / 255.0, 0.0 / 255.0]; // TODO.style
        draw_list.add_rect_filled_multicolor(
            top_left,
            bottom_right,
            outline_color,
            outline_color,
            outline_color,
            outline_color,
        );
    }

    if is_too_small_to_draw {
        ui.set_cursor_screen_pos(bottom_right);
        return;
    }

    {
        // Draw fill
        let mut fill_top_left = top_left;
        let mut fill_bottom_right = bottom_right;
        fill_top_left[0] += outline_size;
        fill_top_left[1] += outline_size;
        fill_bottom_right[0] -= outline_size;
        fill_bottom_right[1] -= outline_size;
        let fill_color = if is_selected {
            [249.0 / 255.0, 212.0 / 255.0, 200.0 / 255.0] // TODO.style
        } else {
            [249.0 / 255.0, 212.0 / 255.0, 35.0 / 255.0] // TODO.style
        };
        draw_list.add_rect_filled_multicolor(
            fill_top_left,
            fill_bottom_right,
            fill_color,
            fill_color,
            fill_color,
            fill_color,
        );

        // Draw name
        if let Some(name) = keyframe.get_frame().file_name() {
            draw_list.with_clip_rect_intersect(fill_top_left, fill_bottom_right, || {
                let text_color = [25.0 / 255.0, 15.0 / 255.0, 0.0 / 255.0]; // TODO.style
                let x = fill_top_left[0] + text_padding;
                let y = (fill_top_left[1] + fill_bottom_right[1]) / 2.0 - 8.0; // TODO.style 8.0 is font_size/2
                let text_position = [x, y];
                draw_list.add_text(text_position, text_color, name.to_string_lossy());
            });
        }
    }

    // Click to select
    {
        let id = format!("frame_button_{}", top_left[0]);
        ui.set_cursor_screen_pos([top_left[0] + resize_handle_size, top_left[1]]);
        if ui.invisible_button(
            &ImString::new(id),
            [
                bottom_right[0] - top_left[0] - resize_handle_size * 2.0,
                bottom_right[1] - top_left[1],
            ],
        ) {
            let new_selection = MultiSelection::process(
                keyframe_index,
                ui.io().key_shift,
                ui.io().key_ctrl,
                &(0..animation.get_num_keyframes()).collect(),
                match &document.view.selection {
                    Some(Selection::Keyframe(s)) => Some(s),
                    _ => None,
                },
            );
            commands.select_keyframes(&new_selection);
        }
    }

    // Drag and drop to re-order interactions
    if !document.is_adjusting_frame_duration() {
        let is_hovering_frame_exact = {
            let id = format!("frame_middle_{}", top_left[0]);
            ui.set_cursor_screen_pos([top_left[0] + resize_handle_size, top_left[1]]);
            ui.invisible_button(&ImString::new(id), [w - resize_handle_size * 2.0, h]);
            ui.is_item_hovered_with_flags(ItemHoveredFlags::ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM)
        };

        let is_mouse_dragging = ui.is_mouse_dragging(MouseButton::Left);
        if document.transient.is_none() && is_mouse_dragging && is_hovering_frame_exact {
            if !is_selected {
                commands.select_keyframes(&MultiSelection::new(vec![keyframe_index]));
            }
            commands.begin_keyframe_drag();
        }
    }

    // Drag to resize interaction
    {
        assert!(resize_handle_size >= 1.0);
        let id = format!("frame_handle_{}", top_left[0]);
        ui.set_cursor_screen_pos([bottom_right[0] - resize_handle_size, top_left[1]]);
        ui.invisible_button(&ImString::new(id), [resize_handle_size * 2.0, h]);
        let is_mouse_dragging = ui.is_mouse_dragging(MouseButton::Left);
        let is_mouse_down = ui.is_mouse_down(MouseButton::Left);
        if document.transient.is_none() {
            if ui.is_item_hovered() {
                ui.set_mouse_cursor(Some(MouseCursor::ResizeEW));
                if is_mouse_down && !is_mouse_dragging {
                    if !is_selected {
                        commands.select_keyframes(&MultiSelection::new(vec![keyframe_index]));
                    }
                    let mouse_pos = ui.io().mouse_pos;
                    let clock_under_mouse =
                        ((mouse_pos[0] - frames_cursor_position_start[0]) / zoom).max(0.0) as u32;
                    commands.begin_keyframe_duration_drag(clock_under_mouse, keyframe_index);
                }
            }
        }
    }

    ui.set_cursor_screen_pos(bottom_right);
}

fn draw_playback_head<'a>(ui: &Ui<'a>, document: &Document, animation: &Animation) {
    let duration = animation.get_duration().unwrap_or(0);

    let now_ms = {
        let now = document.view.timeline_clock;
        let ms = now.as_millis();
        std::cmp::min(ms, duration.into()) as u32
    };

    let zoom = document.view.get_timeline_zoom_factor();
    let draw_list = ui.get_window_draw_list();

    let mut cursor_pos = ui.cursor_screen_pos();
    cursor_pos[0] += now_ms as f32 * zoom;
    let space = ui.content_region_avail();

    let fill_color = [1.0, 0.0 / 255.0, 0.0 / 255.0]; // TODO constants

    draw_list.add_rect_filled_multicolor(
        [cursor_pos[0], cursor_pos[1]],
        [cursor_pos[0] + 1.0, cursor_pos[1] + space[1]],
        fill_color,
        fill_color,
        fill_color,
        fill_color,
    );
}

fn get_frame_under_mouse<'a>(
    ui: &Ui<'a>,
    document: &Document,
    animation: &Animation,
    start_screen_position: [f32; 2],
) -> Option<(usize, FrameLocation)> {
    let mouse_pos = ui.io().mouse_pos;
    let mut cursor = Duration::new(0, 0);
    for (keyframe_index, keyframe) in animation.keyframes_iter().enumerate() {
        let frame_location = get_frame_location(document, cursor, keyframe);
        let frame_start_x = start_screen_position[0] + frame_location.top_left.0;
        if mouse_pos[0] >= frame_start_x && mouse_pos[0] < (frame_start_x + frame_location.size.0) {
            return Some((keyframe_index, frame_location));
        }
        cursor += Duration::from_millis(u64::from(keyframe.get_duration()));
    }
    None
}

fn handle_drag_to_resize<'a>(
    ui: &Ui<'a>,
    commands: &mut CommandBuffer,
    document: &Document,
    frames_cursor_position_start: [f32; 2],
) {
    let is_mouse_dragging = ui.is_mouse_dragging(MouseButton::Left);
    let is_dragging_duration = document.is_adjusting_frame_duration();
    let zoom = document.view.get_timeline_zoom_factor();
    let min_frame_drag_width = 24.0; // TODO.dpi?

    if is_dragging_duration && is_mouse_dragging {
        ui.set_mouse_cursor(Some(MouseCursor::ResizeEW));
        let mouse_pos = ui.io().mouse_pos;
        let clock_under_mouse =
            ((mouse_pos[0] - frames_cursor_position_start[0]) / zoom).max(0.0) as u32;
        let minimum_duration = (min_frame_drag_width / zoom).max(1.0).ceil() as u32;
        commands.update_keyframe_duration_drag(clock_under_mouse, minimum_duration);
    }
}

fn handle_drag_and_drop<'a>(
    ui: &Ui<'a>,
    commands: &mut CommandBuffer,
    document: &Document,
    animation: &Animation,
    cursor_start: [f32; 2],
    cursor_end: [f32; 2],
) {
    let mouse_pos = ui.io().mouse_pos;
    let is_window_hovered =
        ui.is_window_hovered_with_flags(WindowHoveredFlags::ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM);
    let is_mouse_down = ui.is_mouse_down(MouseButton::Left);
    let is_mouse_dragging = ui.is_mouse_dragging(MouseButton::Left);
    if is_window_hovered {
        let dragging_content_frames = document.is_dragging_content_frames();
        let dragging_keyframe = document.is_dragging_timeline_frames();
        let frame_under_mouse = get_frame_under_mouse(ui, document, animation, cursor_start);
        let h = cursor_end[1] - cursor_start[1];

        if is_mouse_dragging {
            match (
                frame_under_mouse,
                dragging_content_frames,
                dragging_keyframe,
            ) {
                (Some((_, frame_location)), true, false)
                | (Some((_, frame_location)), false, true) => {
                    ui.set_cursor_screen_pos([
                        cursor_start[0] + frame_location.top_left.0,
                        cursor_start[1],
                    ]);
                    draw_insert_marker(ui, &ui.get_window_draw_list(), h);
                }
                (None, true, false) | (None, false, true) => {
                    let x = if mouse_pos[0] <= cursor_start[0] {
                        cursor_start[0]
                    } else {
                        cursor_end[0]
                    };
                    ui.set_cursor_screen_pos([x, cursor_start[1]]);
                    draw_insert_marker(ui, &ui.get_window_draw_list(), h);
                }
                _ => (),
            }
        } else if !is_mouse_down {
            match (
                frame_under_mouse,
                dragging_content_frames,
                dragging_keyframe,
            ) {
                (None, true, false) => {
                    let index = if mouse_pos[0] <= cursor_start[0] {
                        0
                    } else {
                        animation.get_num_keyframes()
                    };
                    if let Some(Selection::Frame(paths)) = &document.view.selection {
                        commands
                            .insert_keyframes_before(paths.items.clone().iter().collect(), index);
                    }
                }
                (None, false, true) => {
                    let index = if mouse_pos[0] <= cursor_start[0] {
                        0
                    } else {
                        animation.get_num_keyframes()
                    };
                    commands.reorder_keyframes(index);
                }
                (Some((index, _)), true, false) => {
                    if let Some(Selection::Frame(paths)) = &document.view.selection {
                        commands
                            .insert_keyframes_before(paths.items.clone().iter().collect(), index);
                    }
                }
                (Some((index, _)), false, true) => {
                    commands.reorder_keyframes(index);
                }
                _ => (),
            }
        }
    }
}

pub fn draw<'a>(ui: &Ui<'a>, rect: &Rect<f32>, app_state: &AppState, commands: &mut CommandBuffer) {
    let _style_rounding = ui.push_style_var(WindowRounding(0.0));
    let _style_border = ui.push_style_var(WindowBorderSize(0.0));
    Window::new("Timeline")
        .position(rect.origin.to_array(), Condition::Always)
        .size(rect.size.to_array(), Condition::Always)
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .always_horizontal_scrollbar(true)
        .build(ui, || {
            if let Some(document) = app_state.get_current_document() {
                if let Some(WorkbenchItem::Animation(animation_name)) =
                    &document.view.workbench_item
                {
                    if let Some(animation) = document.sheet.get_animation(animation_name) {
                        if ui.small_button("Play/Pause") {
                            commands.toggle_playback();
                        }
                        ui.same_line();
                        let mut looping = animation.is_looping();
                        if ui.checkbox("Loop", &mut looping) {
                            commands.toggle_looping();
                        }

                        // TODO autoscroll during playback

                        let ticks_cursor_position = ui.cursor_pos();
                        draw_timeline_ticks(ui, commands, document);

                        let frames_cursor_position_start = ui.cursor_screen_pos();
                        let mut frames_cursor_position_end = frames_cursor_position_start;
                        let mut cursor = Duration::new(0, 0);
                        for (keyframe_index, keyframe) in animation.keyframes_iter().enumerate() {
                            ui.set_cursor_screen_pos(frames_cursor_position_start);
                            draw_keyframe(
                                ui,
                                commands,
                                document,
                                animation,
                                keyframe_index,
                                keyframe,
                                frames_cursor_position_start,
                                cursor,
                            );
                            frames_cursor_position_end = ui.cursor_screen_pos();
                            cursor += Duration::from_millis(u64::from(keyframe.get_duration()));
                        }

                        ui.set_cursor_pos(ticks_cursor_position);
                        draw_playback_head(ui, document, animation);

                        handle_drag_to_resize(ui, commands, document, frames_cursor_position_start);

                        handle_drag_and_drop(
                            ui,
                            commands,
                            document,
                            animation,
                            frames_cursor_position_start,
                            frames_cursor_position_end,
                        );

                        if ui.is_window_hovered() && ui.io().key_ctrl {
                            let mouse_wheel = ui.io().mouse_wheel;
                            if mouse_wheel > 0.0 {
                                commands.timeline_zoom_in();
                            } else if mouse_wheel < 0.0 {
                                commands.timeline_zoom_out();
                            }
                        }
                    }
                }
            }
        });
}
