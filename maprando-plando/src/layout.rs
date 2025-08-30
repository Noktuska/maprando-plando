pub mod hotkey_settings;
pub mod info_overlay;
pub mod map_editor_ui;
pub mod render_selection;

pub mod room_search;

pub mod settings_logic;
pub mod settings_customize;

mod settings_gen;

use anyhow::Result;
use egui::{Context, Ui};
use strum_macros::VariantArray;

use crate::layout::{hotkey_settings::HotkeySettingsWindow, info_overlay::InfoOverlayBuilder, render_selection::RenderSelection};

pub struct Layout {
    pub hotkey_settings: HotkeySettingsWindow,
    
    pub render_selection: RenderSelection,
    pub info_overlay_builder: InfoOverlayBuilder,

    pub sidebar_tab: SidebarPanel,

    window_stack: Vec<WindowType>
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum WindowType {
    HotkeySettings
}

#[derive(PartialEq, Eq, Clone, Copy, VariantArray)]
pub enum SidebarPanel {
    Items,
    Rooms,
    Areas,
    Errors,
    Benchmark
}

impl Layout {
    pub fn new() -> Result<Self> {
        Ok(Layout {
            hotkey_settings: HotkeySettingsWindow::new(),
            render_selection: RenderSelection::new()?,
            info_overlay_builder: InfoOverlayBuilder::new(),
            sidebar_tab: SidebarPanel::Items,
            window_stack: Vec::new()
        })
    }

    pub fn render(&mut self, ctx: &Context) {
        let mut windows_to_close = Vec::new();
        for window_type in self.window_stack.iter().cloned() {
            let window: &mut dyn LayoutWindow = match window_type {
                WindowType::HotkeySettings => &mut self.hotkey_settings,
            };

            egui::Window::new(window.get_title()).resizable(false).show(ctx, |ui| {
                if window.render(ctx, ui) {
                    windows_to_close.push(window.get_type());
                }
            });
        }

        for window_type in windows_to_close {
            self.close(window_type);
        }
    }

    pub fn open(&mut self, window_type: WindowType) {
        if self.window_stack.contains(&window_type) {
            return;
        }
        self.window_stack.push(window_type);
    }

    pub fn close(&mut self, window_type: WindowType) {
        self.window_stack.retain(|&x| x != window_type);
    }

    pub fn is_open(&self, window_type: WindowType) -> bool {
        self.window_stack.contains(&window_type)
    }
}

trait LayoutWindow {
    fn render(&mut self, ctx: &Context, ui: &mut Ui) -> bool;
    fn get_title(&self) -> String;
    fn get_type(&self) -> WindowType;
}