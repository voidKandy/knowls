pub mod agent;
pub mod database;
pub mod messages;
use crate::{
    knowledge::parsing::tokens::{vec::TokenVec, Token},
    other_err,
    rpc::lsp::buffer_operations::BufferOpChannelSender,
    server::ServerState,
    MainErr, MainResult,
};
use agent::AgentInteract;
use database::DBInteract;
use lsp_server::RequestId;
use messages::{InteractLspNotification, InteractLspRequest};
use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug, Clone)]
pub enum InteractWrapper {
    Agent(AgentInteract),
    DB(DBInteract),
}

impl<'t> TryFrom<InteractParams<'t>> for InteractWrapper {
    type Error = MainErr;
    fn try_from(value: InteractParams<'t>) -> Result<Self, Self::Error> {
        match (AgentInteract::try_from(value), DBInteract::try_from(value)) {
            (Ok(a), Err(_)) => Ok(Self::Agent(a)),
            (Err(_), Ok(db)) => Ok(Self::DB(db)),
            (Err(ae), Err(dbe)) => {
                Err(other_err!("Could not get either agent or database interact from params\nAgent: {ae:#?}\nDb: {dbe:#?}"))
            }
            _ => {
                unreachable!("should never be able to get both agent and db interact")
            }
        }
    }
}

type InteractParams<'t> = (&'t TokenVec, &'t Token, usize);
trait InteractReadCtx<'i>: std::fmt::Debug {}
trait InteractWriteCtx<'i>: std::fmt::Debug {}

pub enum InteractCtx<'i, T>
where
    T: Interact<'i>,
{
    Read(T::ReadContext),
    Write(T::WriteContext),
}

impl<'i, T> InteractCtx<'i, T>
where
    T: Interact<'i>,
{
    pub fn from_write(w: T::WriteContext) -> Self {
        Self::Write(w)
    }
    pub fn from_read(r: T::ReadContext) -> Self {
        Self::Read(r)
    }
}

pub type ServerStateWriteGuard<'g> = RwLockWriteGuard<'g, ServerState>;
pub type ServerStateReadGuard<'g> = RwLockReadGuard<'g, ServerState>;

pub trait Interact<'i>: for<'t> TryFrom<InteractParams<'t>> {
    type ReadContext: InteractReadCtx<'i>;
    type WriteContext: InteractWriteCtx<'i>;

    fn get_write_context(&self, w: &'i mut ServerStateWriteGuard)
        -> MainResult<Self::WriteContext>;
    fn get_read_context(&self, r: &'i ServerStateReadGuard) -> MainResult<Self::ReadContext>;
    fn diagnostics(&self, ctx: Self::ReadContext) -> Vec<lsp_types::Diagnostic>;

    async fn handle_noti(
        &self,
        noti: impl Into<InteractLspNotification>,
        ctx: &InteractCtx<'i, Self>,
        sender: &mut BufferOpChannelSender,
    ) -> MainResult<()>;
    async fn handle_req(
        &self,
        req: impl Into<InteractLspRequest>,
        rq_id: RequestId,
        ctx: &InteractCtx<'i, Self>,
        sender: &mut BufferOpChannelSender,
    ) -> MainResult<()>;
}
