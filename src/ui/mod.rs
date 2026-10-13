//! Settings UI (Fluent-inspired egui layout matching upstream lolPing).

mod theme;
mod widgets;
pub mod settings_panel;

pub use settings_panel::{NavSection, SettingsHost, SettingsUiState, paint_settings_viewport};
pub use theme::apply_fluent_dark;
