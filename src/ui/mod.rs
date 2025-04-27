pub mod app;
pub mod input;
pub mod output;
pub mod search;
pub mod tasks;
pub mod terminal;
pub mod diff;
pub mod search_view;
pub mod tui;
pub mod logo;

pub use app::App;
pub use output::OutputManager;
pub use tui::render_ui;
pub use logo::{render_logo, render_pixel_logo};