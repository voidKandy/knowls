use crate::{
    agents::{doc_control_role, AgentID, Agents},
    state::SharedState,
};
use eframe::egui;
use egui::{Color32, ScrollArea, SelectableLabel, TextEdit, Ui};
use espionox::prelude::*;
use lsp_types::Uri;
use std::collections::HashSet;
use tracing::warn;

use super::AppSectionState;

#[derive(Debug)]
pub struct EditingAgent {
    id: AgentID,
    system_prompt: String,
    all_other_messages: MessageStack,
    completion_model: CompletionModel,
}

#[derive(Debug)]
pub struct AgentsSectionState {
    all_names: HashSet<AgentID>,
    should_switch_to_agent: Option<AgentID>,
    editing_agent: Option<EditingAgent>,
    try_update_agent: bool,
}

impl EditingAgent {
    fn from_agent_and_id(agent: &Agent, id: impl Into<AgentID>) -> Self {
        let system_prompt = agent
            .cache
            .ref_system_prompt_content()
            .unwrap_or("")
            .trim()
            .to_string();

        let all_other_messages: MessageStack = agent
            .cache
            .ref_filter_by(&MessageRole::System, false)
            .into();

        Self {
            id: id.into(),
            system_prompt,
            all_other_messages,
            completion_model: agent.completion_model.clone(),
        }
    }

    /// Assumes self is more up to date than agent, if they are out of sync, updates agent to the
    /// state of self
    fn try_sync_with_agent(&self, agent: &mut Agent) {
        if let Some(system_prompt) = agent.cache.mut_system_prompt_content() {
            if self.system_prompt != *system_prompt {
                *system_prompt = self.system_prompt.clone();
            }
        }
        if agent.completion_model != self.completion_model {
            agent.completion_model = self.completion_model.clone();
        }
    }
}

impl Default for AgentsSectionState {
    fn default() -> Self {
        Self {
            all_names: HashSet::new(),
            should_switch_to_agent: Some(AgentID::Global),
            editing_agent: None,
            try_update_agent: false,
        }
    }
}

fn get_all_names(agents: &Agents) -> HashSet<AgentID> {
    let mut all_names = HashSet::new();
    all_names.insert(AgentID::Global);

    for id in all_custom_names(agents) {
        all_names.insert(id.clone());
    }

    for id in all_doc_names(agents) {
        all_names.insert(id.clone());
    }

    all_names
}

fn all_custom_names(agents: &Agents) -> Vec<&AgentID> {
    agents
        .iter_agents()
        .filter_map(|(id, _)| {
            if let AgentID::Char(_) = id {
                Some(id)
            } else {
                None
            }
        })
        .collect::<Vec<&AgentID>>()
}
fn all_doc_names(agents: &Agents) -> Vec<&AgentID> {
    agents
        .iter_agents()
        .filter_map(|(id, _)| {
            if let AgentID::Uri(_) = id {
                Some(id)
            } else {
                None
            }
        })
        .collect::<Vec<&AgentID>>()
}

impl AgentsSectionState {
    pub fn update(&mut self, agents: &mut Agents) {
        let get_global = |a: &mut Agents| -> EditingAgent {
            let id = AgentID::Global;
            EditingAgent::from_agent_and_id(
                a.get_agent_ref(id.clone()).expect("No global agent?"),
                id,
            )
        };

        let all_names = get_all_names(agents);
        if all_names != self.all_names {
            warn!("updating all names: {all_names:#?}");
            self.all_names = all_names
        }

        if let Some(switch_to_agent) = self.should_switch_to_agent.take() {
            self.editing_agent = match switch_to_agent {
                AgentID::Global => Some(get_global(agents)),
                AgentID::Char(_) => {
                    let char: char = switch_to_agent.try_into().expect("failed to get char");
                    agents
                        .get_agent_ref(char)
                        .and_then(|ag| Some(EditingAgent::from_agent_and_id(ag, char)))
                }
                AgentID::Uri(_) => {
                    let uri: Uri = switch_to_agent.try_into().expect("failed to get uri");
                    agents
                        .get_agent_ref(&uri)
                        .and_then(|ag| Some(EditingAgent::from_agent_and_id(ag, &uri)))
                }
            };

            if self.editing_agent.is_none() {
                self.editing_agent = Some(get_global(agents));
            }
        }
    }

    fn current_agent_id(&self) -> Option<&AgentID> {
        Some(&self.editing_agent.as_ref()?.id)
    }
}

impl AppSectionState for AgentsSectionState {
    fn render(&mut self, ui: &mut Ui, mut state: SharedState) {
        let mut guard = state.get_write().unwrap();
        match guard.agents.as_mut() {
            Some(agents) => {
                self.update(agents);

                let selectable_labels =
                    |current_name: &AgentID| -> Vec<(SelectableLabel, &AgentID)> {
                        self.all_names
                            .iter()
                            .map(|n| {
                                (
                                    egui::SelectableLabel::new(current_name == n, format!("{n}")),
                                    n,
                                )
                            })
                            .collect()
                    };

                if let Some(current_agent_name) = self.current_agent_id() {
                    for (label, name) in selectable_labels(&current_agent_name) {
                        ui.horizontal_top(|ui| {
                            if ui.add(label).clicked() {
                                warn!("clicked label. Changing current name to {name:#?}");
                                self.should_switch_to_agent = Some(name.to_owned());
                            }
                        });
                    }
                }

                if let Some(editing) = self.editing_agent.as_mut() {
                    ui.vertical_centered_justified(|ui| {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.label("System Prompt");
                            let textedit = TextEdit::multiline(&mut editing.system_prompt)
                                .interactive(true)
                                .min_size(egui::Vec2 { x: 25., y: 1. });
                            if ui.add(textedit).changed() {
                                self.try_update_agent = true;
                            }

                            if self.try_update_agent {
                                if ui.button("Save").clicked() {
                                    if let Some(agent) = agents.get_agent_mut(&editing.id) {
                                        warn!("trying to update agent {agent:#?}");
                                        editing.try_sync_with_agent(agent);
                                        self.try_update_agent = false;
                                    }
                                }
                            }

                            for message in editing.all_other_messages.as_ref().iter() {
                                ui.label(message.role.to_string().to_uppercase());
                                let mut content = message.content.clone();
                                let singleline = {
                                    if content.lines().count() < 2 {
                                        true
                                    } else if content.lines().count() < 3
                                        && content
                                            .lines()
                                            .into_iter()
                                            .nth(2)
                                            .is_some_and(|l| l.trim().is_empty())
                                    {
                                        true
                                    } else {
                                        false
                                    }
                                };
                                let color = match message.role {
                                    MessageRole::User => Color32::from_rgb(255, 223, 223),
                                    MessageRole::Assistant => Color32::from_rgb(210, 220, 255),
                                    _ if message.role == doc_control_role() => {
                                        Color32::from_rgb(200, 0, 198)
                                    }
                                    _ => Color32::from_rgb(255, 224, 230),
                                };

                                let textedit = match singleline {
                                    true => TextEdit::singleline(&mut content)
                                        .interactive(false)
                                        .code_editor()
                                        .frame(false)
                                        .text_color(color),
                                    false => TextEdit::multiline(&mut content)
                                        .interactive(false)
                                        .code_editor()
                                        .frame(false)
                                        .text_color(color)
                                        .min_size(egui::Vec2 { x: 25., y: 1. }),
                                };
                                ui.add(textedit);
                            }
                        });
                    });
                }
            }
            None => {
                ui.label("No Agents");
            }
        }
    }
}
