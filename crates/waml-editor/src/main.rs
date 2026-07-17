pub use makepad_widgets;
use makepad_widgets::*;

mod app;
mod camera;
mod canvas;
mod cli;
mod load;
mod scene;
mod sizing;
mod tree;

use app::App;

app_main!(App);
