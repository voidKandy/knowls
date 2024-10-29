use egui::{Spinner, Ui};

use crate::state::SharedState;

use super::AppSectionState;

#[derive(Debug, Default)]
pub struct HomeSectionState;

impl AppSectionState for HomeSectionState {
    fn render(&mut self, ui: &mut Ui, state: SharedState) {
        ui.label("Welcome");
    }
}
