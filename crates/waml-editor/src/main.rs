pub use makepad_widgets;
use makepad_widgets::*;

mod app;
mod camera;
mod canvas;
mod cli;
mod doc_tabs;
mod inspector;
mod inspector_panel;
mod load;
mod scene;
mod selection_toolbar;
mod sizing;
mod statusbar;
mod tool_dock;
mod tree;
mod tree_panel;

use app::App;

app_main!(App);
