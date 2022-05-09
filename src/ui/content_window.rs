use imgui::StyleVar::*;
use imgui::*;
use std::ffi::OsStr;

use crate::sheet::{Animation, Frame};
use crate::state::*;
use crate::ui::Rect;

fn draw_tabs<'a>(ui: &Ui<'a>, commands: &mut CommandBuffer) {
    if ui.small_button("Frames") {
        commands.switch_to_content_tab(ContentTab::Frames);
    }
    ui.same_line();
    if ui.small_button("Animations") {
        commands.switch_to_content_tab(ContentTab::Animations);
    }
}

fn draw_frames<'a>(ui: &Ui<'a>, commands: &mut CommandBuffer, document: &Document) {
    if ui.small_button("Import…") {
        commands.import(document);
    }
    let mut frames: Vec<(&OsStr, &Frame)> = document
        .sheet
        .frames_iter()
        .filter_map(|f| {
            if let Some(name) = f.get_source().file_name() {
                Some((name, f))
            } else {
                None
            }
        })
        .collect();
    frames.sort_unstable();
    for (name, frame) in frames.iter() {
        let is_selected = document.is_frame_selected(frame);

        if Selectable::new(&ImString::new(name.to_string_lossy()))
            .selected(is_selected)
            .flags(SelectableFlags::ALLOW_DOUBLE_CLICK)
            .size([0.0, 0.0])
            .build(ui)
        {
            if ui.is_mouse_double_clicked(MouseButton::Left) {
                commands.edit_frame(frame);
            } else {
                let new_selection = MultiSelection::process(
                    frame.get_source().to_owned(),
                    ui.io().key_shift,
                    ui.io().key_ctrl,
                    &frames
                        .iter()
                        .map(|(_, f)| f.get_source().to_owned())
                        .collect(),
                    match &document.view.selection {
                        Some(Selection::Frame(s)) => Some(s),
                        _ => None,
                    },
                );
                commands.select_frames(&new_selection);
            }
        } else if document.transient.is_none()
            && ui.is_item_active()
            && ui.is_mouse_dragging(MouseButton::Left)
        {
            if !is_selected {
                commands.select_frames(&MultiSelection::new(vec![frame.get_source().to_owned()]));
            }
            commands.begin_frames_drag();
        }
    }
}

fn draw_animations<'a>(ui: &Ui<'a>, commands: &mut CommandBuffer, document: &Document) {
    if ui.small_button("Add") {
        commands.create_animation();
    }
    let mut animations: Vec<&Animation> = document.sheet.animations_iter().collect();
    animations.sort_unstable();
    for animation in animations.iter() {
        let is_selected = document.is_animation_selected(animation);
        if Selectable::new(&ImString::new(animation.get_name()))
            .flags(SelectableFlags::ALLOW_DOUBLE_CLICK)
            .selected(is_selected)
            .size([0.0, 0.0])
            .build(ui)
        {
            if ui.is_mouse_double_clicked(MouseButton::Left) {
                commands.edit_animation(animation);
            } else {
                let new_selection = MultiSelection::process(
                    animation.get_name().to_owned(),
                    ui.io().key_shift,
                    ui.io().key_ctrl,
                    &animations.iter().map(|a| a.get_name().to_owned()).collect(),
                    match &document.view.selection {
                        Some(Selection::Animation(s)) => Some(s),
                        _ => None,
                    },
                );
                commands.select_animations(&new_selection);
            }
        }
    }
}

pub fn draw<'a>(ui: &Ui<'a>, rect: &Rect<f32>, app_state: &AppState, commands: &mut CommandBuffer) {
    let _style_rounding = ui.push_style_var(WindowRounding(0.0));
    let _style_border = ui.push_style_var(WindowBorderSize(0.0));
    Window::new("Content")
        .position(rect.min().to_array(), Condition::Always)
        .size(rect.size.to_array(), Condition::Always)
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .build(ui, || {
            // TODO draw something before document is loaded?
            if let Some(document) = app_state.get_current_document() {
                draw_tabs(ui, commands);
                ui.separator();
                match document.view.content_tab {
                    ContentTab::Frames => draw_frames(ui, commands, document),
                    ContentTab::Animations => draw_animations(ui, commands, document),
                }
            }
        });
}
