use espionox::agents::Agent;
use lsp_server::RequestId;
use lsp_types::{DiagnosticSeverity, HoverContents, Range};

use crate::{
    agents::{message_stack_into_marked_string, AgentID},
    knowledge::parsing::tokens::Token,
    other_err,
    rpc::lsp::buffer_operations::{BufferOpChannelSender, BufferOperation},
    MainErr, MainResult,
};

use super::{
    messages::InteractLspNotification, messages::InteractLspRequest, Interact, InteractCtx,
    InteractParams, InteractReadCtx, InteractWriteCtx, ServerStateReadGuard, ServerStateWriteGuard,
};

#[derive(Debug, Clone, PartialEq)]
pub enum AgentInteractVar {
    // the string value to lock into agent's context
    Push(Option<String>),
    // user's prompt
    Prompt(Option<String>),
}

#[derive(Debug)]
pub struct AgentReadCtx<'i> {
    agent: &'i Agent,
}
impl<'i> InteractReadCtx<'i> for AgentReadCtx<'i> {}

#[derive(Debug)]
pub struct AgentWriteCtx<'i> {
    agent: &'i mut Agent,
}
impl<'i> InteractWriteCtx<'i> for AgentWriteCtx<'i> {}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentInteract {
    range: Range,
    agent_id: AgentID,
    var: AgentInteractVar,
}

impl AgentInteract {
    const PUSH: char = '+';
    const PROMPT: char = '@';
}

impl<'t> TryFrom<InteractParams<'t>> for AgentInteract {
    type Error = MainErr;
    fn try_from((toks, tok, idx): InteractParams<'t>) -> Result<Self, Self::Error> {
        if let Token::Comment(parsed) = tok {
            let mut chars = parsed.content.trim().chars();

            match chars.next().ok_or(other_err!("empty content"))? {
                Self::PUSH => {
                    let agent_id_char = chars.next().expect("no agent id");
                    let agent_id = AgentID::try_from_char(agent_id_char)
                        .ok_or(other_err!("{agent_id_char} is not a valid agent id"))?;

                    let block = match toks.get(idx + 1) {
                        Some(Token::Block(block)) => Some(block.to_string()),
                        _ => None,
                    };
                    return Ok(Self {
                        range: parsed.range,
                        agent_id,
                        var: AgentInteractVar::Push(block),
                    });
                }
                Self::PROMPT => {
                    let agent_id_char = chars.next().expect("no agent id");
                    let agent_id = AgentID::try_from_char(agent_id_char)
                        .ok_or(other_err!("{agent_id_char} is not a valid agent id"))?;
                    let prompt = chars.collect::<String>().trim().to_string();
                    let prompt = if prompt.is_empty() {
                        None
                    } else {
                        Some(prompt)
                    };

                    return Ok(Self {
                        range: parsed.range,
                        agent_id,
                        var: AgentInteractVar::Prompt(prompt),
                    });
                }
                c => return Err(other_err!("{c} is not valid for an agent interact")),
            }
        } else {
            return Err(other_err!("Wrong token Variant"));
        }
    }
}

impl<'i> Interact<'i> for AgentInteract {
    type ReadContext = AgentReadCtx<'i>;
    type WriteContext = AgentWriteCtx<'i>;

    async fn handle_noti(
        &self,
        noti: impl Into<InteractLspNotification>,
        _ctx: &InteractCtx<'i, Self>,
        _sender: &mut BufferOpChannelSender,
    ) -> MainResult<()> {
        match Into::<InteractLspNotification>::into(noti) {
            InteractLspNotification::Save(_save) => {}
            InteractLspNotification::Change(_change) => {}
            InteractLspNotification::Open(_open) => {}
        }
        Ok(())
    }

    async fn handle_req(
        &self,
        req: impl Into<InteractLspRequest>,
        rq_id: RequestId,
        ctx: &InteractCtx<'i, Self>,
        sender: &mut BufferOpChannelSender,
    ) -> MainResult<()> {
        match Into::<InteractLspRequest>::into(req) {
            InteractLspRequest::Hover(_hover) => {
                let messages: Vec<&espionox::prelude::Message> = match ctx {
                    InteractCtx::Read(r) => r.agent.cache.as_ref().iter().rev().take(5).collect(),
                    InteractCtx::Write(w) => w.agent.cache.as_ref().iter().rev().take(5).collect(),
                };
                let stack = espionox::agents::memory::MessageStackRef::from(messages);
                let contents = HoverContents::Scalar(message_stack_into_marked_string(stack));
                sender
                    .send_operation(BufferOperation::HoverResponse {
                        id: rq_id,
                        contents,
                    })
                    .await?;
            }
            InteractLspRequest::GotoDef(_goto) => {}
            InteractLspRequest::Diagnostic(_diag) => {}
        };
        Ok(())
    }

    fn diagnostics(&self, _wctx: Self::ReadContext) -> Vec<lsp_types::Diagnostic> {
        let mut all_diagnostics = vec![];
        let severity = Some(DiagnosticSeverity::HINT);
        let message = match self.var {
            AgentInteractVar::Push(_) => "Push",
            AgentInteractVar::Prompt(_) => "Prompt",
        }
        .to_string();
        all_diagnostics.push(lsp_types::Diagnostic {
            range: self.range,
            severity,
            message,
            ..Default::default()
        });

        all_diagnostics
    }

    fn get_read_context(&self, r: &'i ServerStateReadGuard) -> MainResult<Self::ReadContext> {
        let agent = r.agents.get_agent_ref(&self.agent_id).ok_or(other_err!(
            "server has no agent with id {:#?}",
            self.agent_id
        ))?;
        Ok(AgentReadCtx { agent })
    }

    fn get_write_context(
        &self,
        w: &'i mut ServerStateWriteGuard,
    ) -> MainResult<Self::WriteContext> {
        let agent = w.agents.get_agent_mut(&self.agent_id).ok_or(other_err!(
            "server has no agent with id {:#?}",
            self.agent_id
        ))?;

        Ok(AgentWriteCtx { agent })
    }
}

mod tests {
    use espionox::agents::Agent;
    use lsp_types::Position;

    use crate::knowledge::parsing::tokens::{vec::TokenVec, Token};

    use super::{AgentInteract, Interact};

    #[test]
    fn correctly_parses_interacts() {
        let range = lsp_types::Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 12,
            },
        };
        let tok = Token::Comment(crate::knowledge::parsing::comments::ParsedComment {
            content: "@_ someprompt".to_string(),
            range: range.clone(),
        });
        let toks = TokenVec::new(vec![tok.clone()], vec![0]);
        let interact = AgentInteract::try_from((&toks, &tok, 0)).unwrap();

        let expected_interact = AgentInteract {
            range,
            var: crate::interact::agent::AgentInteractVar::Prompt(Some("someprompt".to_string())),
            agent_id: crate::agents::AgentID::try_from_char('_').unwrap(),
        };

        assert_eq!(interact, expected_interact);
    }
}
