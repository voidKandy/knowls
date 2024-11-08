mod agents;
mod database;
mod documents;
mod home;
use crate::state::SharedState;
use agents::AgentsSectionState;
use database::DBSectionState;
use documents::DocumentSectionState;
use eframe::egui;
use egui::{Color32, Layout, RichText, SelectableLabel, Spinner, Ui};
use home::HomeSectionState;

pub fn run_gui(state: SharedState<'static>) -> eframe::Result {
    let options = eframe::NativeOptions {
        run_and_return: true,
        viewport: egui::ViewportBuilder::default().with_inner_size([1080.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ESPX - LS",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(state)))),
    )
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum UiSection {
    #[default]
    Home,
    Agents,
    Documents,
    Database,
}

impl AsRef<str> for UiSection {
    fn as_ref(&self) -> &str {
        match self {
            Self::Home => "Home",
            Self::Agents => "Agents",
            Self::Documents => "Documents",
            Self::Database => "Database",
        }
    }
}

impl UiSection {
    fn all_variants() -> Vec<Self> {
        vec![Self::Home, Self::Agents, Self::Documents, Self::Database]
    }

    fn into_section_state(&self) -> Box<dyn AppSectionState + 'static> {
        match self {
            Self::Home => Box::new(HomeSectionState::default()),
            Self::Agents => Box::new(AgentsSectionState::default()),
            Self::Database => Box::new(DBSectionState::default()),
            Self::Documents => Box::new(DocumentSectionState::default()),
        }
    }
}

struct UiSectionSelection {
    section: UiSection,
    state: Box<dyn AppSectionState>,
}

impl Default for UiSectionSelection {
    fn default() -> Self {
        let section = UiSection::default();
        let state = section.into_section_state();
        Self { section, state }
    }
}

impl UiSectionSelection {
    fn change_to(&mut self, section: UiSection) {
        self.state = section.into_section_state();
        self.section = section;
    }
}

struct App {
    state: SharedState<'static>,
    selected_section: UiSectionSelection,
}

impl App {
    fn new(state: SharedState<'static>) -> Self {
        Self {
            state,
            selected_section: UiSectionSelection::default(),
        }
    }
}

trait AppSectionState: std::fmt::Debug {
    fn render(&mut self, ui: &mut Ui, state: SharedState<'static>);
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("Left Panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
                    for sect in UiSection::all_variants() {
                        let name = sect.as_ref().to_string();
                        let label =
                            SelectableLabel::new(self.selected_section.section == sect, name);
                        if ui.add(label).clicked() {
                            self.selected_section.change_to(sect);
                        }
                    }
                });
            });

        egui::TopBottomPanel::top("Header").show(ctx, |ui| {
            if let Some(r) = self.state.0.try_read().ok() {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::Frame::default().inner_margin(4.0).show(ui, |ui| {
                        match r.attached.as_ref() {
                            Some(add) => {
                                let richtext = RichText::new("✅").size(20.).color(Color32::GREEN);
                                ui.label(richtext)
                                    .on_hover_text(format!("LSP attached at: {add:#?}"));
                            }
                            None => {
                                let spinner = Spinner::new().size(20.).color(Color32::ORANGE);
                                ui.add(spinner).on_hover_text("LSP is not attached");
                            }
                        }
                    });
                });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.selected_section.state.render(ui, self.state.clone());
        });
    }
}
