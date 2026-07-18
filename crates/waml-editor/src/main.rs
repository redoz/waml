pub use makepad_widgets;
use makepad_widgets::*;

mod app;
mod camera;
mod canvas;
mod card;
mod cli;
mod config;
mod diagram_switcher;
mod doc_tabs;
mod draw_hud;
mod inspector;
mod inspector_panel;
mod load;
mod node_style;
mod scene;
mod selection_toolbar;
mod shortcuts_overlay;
mod sizing;
mod statusbar;
mod theme_atlas;
mod tool_dock;
mod tree;
mod tree_panel;

use app::App;

app_main!(App);
