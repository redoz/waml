pub use makepad_widgets;
use makepad_widgets::*;

mod app;
mod app_menu;
mod camera;
mod canvas;
mod card;
mod caption_button;
mod cli;
mod config;
mod action_link;
mod diagram_switcher;
mod doc_tabs;
mod frame;
mod icons;
mod inspector;
mod inspector_panel;
mod load;
mod logo;
mod node_design_editor;
mod node_style;
mod radial;
mod recent_row;
mod scene;
mod selection_toolbar;
mod shortcuts_overlay;
mod sizing;
mod start_screen;
mod statusbar;
mod theme_atlas;
mod tool_dock;
mod tree;
mod tree_panel;

use app::App;

app_main!(App);
