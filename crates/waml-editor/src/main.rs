pub use makepad_widgets;
use makepad_widgets::*;

mod app;
mod camera;
mod canvas;
mod load;
mod scene;
mod sizing;

use app::App;

app_main!(App);
