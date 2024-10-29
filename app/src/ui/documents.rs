use egui::{Layout, RichText, SelectableLabel, TextEdit, Ui};
use egui_extras::{Size, StripBuilder};
use lsp_types::Uri;

use crate::state::SharedState;

use super::AppSectionState;

#[derive(Debug, Default)]
pub struct DocumentSectionState {
    current_document: Option<Uri>,
}

impl AppSectionState for DocumentSectionState {
    fn render(&mut self, ui: &mut Ui, state: SharedState) {
        let r = state.get_read().unwrap();
        let all_documents: Vec<&Uri> = r.documents.keys().collect();

        let w = ui.available_width() / 4.;
        StripBuilder::new(ui)
            .size(Size::exact(w)) // top cell
            .vertical(|mut strip| {
                strip.strip(|builder| {
                    builder.sizes(Size::remainder(), 2).horizontal(|mut strip| {
                        strip.cell(|ui| {
                            for uri in all_documents {
                                let name = uri.to_string();
                                let label = SelectableLabel::new(
                                    self.current_document.as_ref() == Some(uri),
                                    name,
                                );
                                if ui.add(label).clicked() {
                                    self.current_document = Some(uri.clone());
                                }
                            }
                        });
                        strip.cell(|ui| match self.current_document.as_ref() {
                            None => {
                                ui.label("No Document Selected");
                            }
                            Some(uri) => match r.documents.get(&uri) {
                                None => {
                                    ui.label(format!("{uri:#?} Has no tokens"));
                                }
                                Some(tokens) => {
                                    let mut val = tokens.to_string();
                                    let textedit = TextEdit::multiline(&mut val)
                                        .interactive(false)
                                        .code_editor();
                                    ui.add(textedit);
                                }
                            },
                        });
                    });
                });
            });
    }
}
