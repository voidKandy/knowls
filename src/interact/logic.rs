use super::{agent::AgentInteract, database::DBInteract, execution::InteractDocumentInfo};
use crate::{
    knowledge::parsing::comments::ParsedComment,
    rpc::lsp::buffer_operations::BufferOpChannelSender, server::ServerState, util::Diff,
    MainResult,
};
use lsp_server::RequestId;
use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticParams, GotoDefinitionParams, HoverParams,
};
use tokio::sync::RwLockWriteGuard;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Interact<'i> {
    pub variant: InteractVar,
    pub parsed_args: Vec<InteractArg>,
    _marker: std::marker::PhantomData<&'i ()>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InteractVar {
    Agent(AgentInteract),
    DB(DBInteract),
}

impl InteractVar {
    pub const AGENT_PUSH: Self = Self::Agent(AgentInteract::Push);
    pub const AGENT_PROMPT: Self = Self::Agent(AgentInteract::Prompt);
    pub const AGENT_RAG_PROMPT: Self = Self::Agent(AgentInteract::RagPrompt);
    pub const DATABASE_STATE: Self = Self::DB(DBInteract);
}

pub type ServerStateWriteGuard<'g> = RwLockWriteGuard<'g, ServerState>;
pub trait LspMessageInteract<'i, 'g, ARGS>: std::fmt::Debug + Copy {
    fn diagnostics(&self, args: ARGS) -> Vec<Diagnostic>;

    fn get_execution_args(
        &self,
        w: &'i mut ServerStateWriteGuard<'g>,
        interact_comment: &'i ParsedComment,
        doc_info: InteractDocumentInfo<'i>,
        args: &Vec<InteractArg>,
    ) -> Option<ARGS>;

    async fn execute_request(
        &self,
        args: ARGS,
        rq_id: RequestId,
        params: impl Into<InteractLspRequest>,
        sender: &mut BufferOpChannelSender,
    ) -> MainResult<()>;
    async fn execute_notification(
        &self,
        args: ARGS,
        noti: impl Into<InteractLspNotification>,
        sender: &mut BufferOpChannelSender,
    ) -> MainResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InteractArg {
    Char(char),
    String(String),
}

impl InteractArg {
    pub fn as_char(&self) -> Option<&char> {
        match self {
            Self::Char(ch) => Some(ch),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(str) => Some(str),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum InteractLspMessage {
    Req {
        body: InteractLspRequest,
        id: RequestId,
    },
    Noti(InteractLspNotification),
}
impl From<(InteractLspRequest, RequestId)> for InteractLspMessage {
    fn from((body, id): (InteractLspRequest, RequestId)) -> Self {
        Self::Req { body, id }
    }
}
impl From<InteractLspNotification> for InteractLspMessage {
    fn from(value: InteractLspNotification) -> Self {
        Self::Noti(value)
    }
}

#[derive(Debug, Clone)]
pub enum InteractLspRequest {
    GotoDef(GotoDefinitionParams),
    Hover(HoverParams),
    Diagnostic(DocumentDiagnosticParams),
}
impl Into<InteractLspRequest> for GotoDefinitionParams {
    fn into(self) -> InteractLspRequest {
        InteractLspRequest::GotoDef(self)
    }
}
impl Into<InteractLspRequest> for HoverParams {
    fn into(self) -> InteractLspRequest {
        InteractLspRequest::Hover(self)
    }
}
impl Into<InteractLspRequest> for DocumentDiagnosticParams {
    fn into(self) -> InteractLspRequest {
        InteractLspRequest::Diagnostic(self)
    }
}

#[derive(Debug, Clone)]
pub enum InteractLspNotification {
    Save(DidSaveTextDocumentParams),
    Change(DidChangeTextDocumentParams),
    Open(DidOpenTextDocumentParams),
}
impl Into<InteractLspNotification> for DidSaveTextDocumentParams {
    fn into(self) -> InteractLspNotification {
        InteractLspNotification::Save(self)
    }
}
impl Into<InteractLspNotification> for DidChangeTextDocumentParams {
    fn into(self) -> InteractLspNotification {
        InteractLspNotification::Change(self)
    }
}
impl Into<InteractLspNotification> for DidOpenTextDocumentParams {
    fn into(self) -> InteractLspNotification {
        InteractLspNotification::Open(self)
    }
}

impl<'i> Interact<'i> {
    pub fn new(variant: InteractVar, parsed_args: Vec<InteractArg>) -> Self {
        Self {
            variant,
            parsed_args,
            _marker: std::marker::PhantomData,
        }
    }
    pub fn try_from_str(str: &str) -> Option<Self> {
        warn!("trying to build interact from str: {str}");
        let first_non_whitespace_pos = str.chars().position(|c| !c.is_whitespace())?;
        let command_char = str.chars().nth(first_non_whitespace_pos)?;

        let (is_agent_int, is_state_int) = (
            AgentInteract::try_from(command_char).is_ok(),
            DBInteract::try_from(command_char).is_ok(),
        );

        if !is_agent_int && !is_state_int {
            warn!("could not get interact from input string");
            return None;
        }

        warn!("command_char: {command_char}");
        if is_state_int && is_agent_int {
            panic!("somehow got both agent and state interact")
        }

        let after_str = str
            .chars()
            .skip(first_non_whitespace_pos + 1)
            .collect::<String>();

        let parsed_args = after_str
            .split_whitespace()
            .map(|slice| {
                if slice.chars().count() == 1 {
                    InteractArg::Char(slice.chars().next().unwrap())
                } else {
                    match slice.chars().position(|c| c == '\n') {
                        Some(pos) => InteractArg::String(slice[..pos].to_string()),
                        None => InteractArg::String(slice.to_string()),
                    }
                }
            })
            .collect();

        let variant = {
            if is_state_int {
                InteractVar::DB(DBInteract::try_from(command_char).unwrap())
            } else if is_agent_int {
                InteractVar::Agent(AgentInteract::try_from(command_char).unwrap())
            } else {
                unreachable!();
            }
        };

        Some(Self::new(variant, parsed_args))
    }
}

mod tests {
    use lsp_types::Position;

    use crate::interact::logic::InteractVar;

    use super::{AgentInteract, Interact, InteractArg};

    #[test]
    fn correctly_parses_interacts() {
        let input = "@_ someprompt";
        let interact = Interact::try_from_str(input).unwrap();
        let expected_args = vec![
            InteractArg::Char('_'),
            InteractArg::String("someprompt".to_string()),
        ];

        let expected_interact = InteractVar::Agent(AgentInteract::Prompt);

        assert_eq!(interact.variant, expected_interact);
        assert_eq!(interact.parsed_args, expected_args);
    }
}
