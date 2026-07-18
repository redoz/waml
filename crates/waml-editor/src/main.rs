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
mod sizing;
mod tree;
mod tree_panel;

use app::App;

app_main!(App);
