use crate::{
    agents::{AgentID, Agents},
    config::Config,
    database::Database,
    interact::parsing::tokens::vec::TokenVec,
    other_err,
    sockets::rpc::{ServerRPCWrapper, ServerRelayResponse},
    MainResult,
};
use seraphic::{RpcRequest, RpcRequestWrapper, RpcResponse};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, Interest},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpListener, TcpStream, ToSocketAddrs,
    },
    sync::RwLock,
    task::JoinHandle,
};

#[derive(Debug)]
pub struct Server<'s> {
    listener: TcpListener,
    state: Arc<RwLock<ServerState<'s>>>,
    connections: HashMap<String, JoinHandle<()>>,
}

pub struct TcpPacket;

impl TcpPacket {
    const HEADER_SIZE: usize = std::mem::size_of::<u32>() / std::mem::size_of::<u8>();

    pub fn serialize(mes: impl Into<seraphic::socket::Message>) -> Vec<u8> {
        let payload = Into::<seraphic::socket::Message>::into(mes);
        let vec = serde_json::to_vec(&payload).unwrap();
        let size: u32 = vec.len() as u32;

        tracing::warn!("serialized payload of size: {size}");

        let mut buff = Vec::with_capacity(Self::HEADER_SIZE + vec.len());
        buff.extend_from_slice(&size.to_le_bytes());
        buff.extend_from_slice(&vec);
        buff
    }

    pub async fn read_from_stream<R>(reader: &mut R) -> tokio::io::Result<seraphic::socket::Message>
    where
        R: AsyncReadExt + Unpin,
    {
        let mut header_buf = [0u8; Self::HEADER_SIZE];
        reader
            .read_exact(&mut header_buf)
            .await
            .expect("failed to read header");

        let payload_size = u32::from_le_bytes(header_buf) as usize;
        tracing::warn!("expecting payload of size: {payload_size}");

        let mut payload_buf = vec![0; payload_size];
        reader
            .read_exact(&mut payload_buf)
            .await
            .expect("failed to read payload");

        let message: seraphic::socket::Message = serde_json::from_slice(&payload_buf)
            .map_err(|e| tokio::io::Error::new(tokio::io::ErrorKind::InvalidData, e))?;

        Ok(message)
    }
}

// i think knowledge could be generalized more to add different kinds of knowledge bases
// also, i think it's time to use treesitter
#[derive(Debug)]
enum Knowledge<'s> {
    Document(TokenVec<'s>),
}

#[derive(Debug)]
pub struct ServerState<'s> {
    config: Config,
    db: Option<Database>,
    agents: Agents,
    knowledge: HashMap<surrealdb::sql::Id, Knowledge<'s>>,
}

#[derive(Debug)]
pub struct Client {
    pub stream: TcpStream,
}

impl Client {
    #[tracing::instrument(name = "client connecting", skip_all)]
    pub async fn connect(addr: impl ToSocketAddrs) -> MainResult<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self { stream })
    }

    pub async fn send(&mut self, r: impl RpcRequest, id: &str) -> MainResult<()> {
        let req: seraphic::socket::Request = r.into_rpc_request(id)?;
        tracing::warn!("client sending: {req:#?}");
        let packet = TcpPacket::serialize(req);

        self.stream
            .write(&packet)
            .await
            .map_err(|e| other_err!("problem writing to stream on clientside: {e:#?}"))?;

        self.stream
            .flush()
            .await
            .map_err(|e| other_err!("failed to flush stream: {e:#?}"))
    }
}

impl<'s> ServerState<'s> {
    pub async fn from_config(config: Config) -> Self {
        let db = match &config.database {
            Some(db_config) => Some(
                Database::new(db_config.clone())
                    .await
                    .expect("failed to get database"),
            ),
            None => None,
        };

        tracing::warn!("got database");

        let mut agents = Agents::from(&config.model);
        tracing::warn!("got agents");
        if let Some(ref agents_config) = &config.agents {
            for (agent_id, agent_settings) in agents_config.clone().into_iter() {
                match agent_id {
                    AgentID::Uri(uri_str) => {
                        tracing::warn!("Did not expect to encounter a uri agent here, encountered: {uri_str:#?}")
                    }
                    AgentID::Global => {
                        let mut global_agent = agents.remove(agent_id.clone()).expect("No global?");
                        agent_settings.change_agent(&mut global_agent);
                        agents.insert(agent_id, global_agent);
                    }
                    AgentID::Char(_) => {
                        let agent =
                            crate::agents::inits::custom(&config.model, agent_settings.sys_prompt);
                        agents.insert(agent_id, agent);
                    }
                }
            }
        }

        Self {
            db,
            agents,
            config,
            knowledge: HashMap::new(),
        }
    }
}

#[derive(Debug)]
struct ConnectionThreadState<'c> {
    read: ReadHalf<'c>,
    write: WriteHalf<'c>,
    agents: Agents,
    knowledge: HashMap<surrealdb::sql::Id, Knowledge<'c>>,
}

impl<'c> ConnectionThreadState<'c> {
    const THREAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

    fn new(stream: &'c mut TcpStream) -> Self {
        let (read, write) = stream.split();
        Self {
            read,
            write,
            agents: Agents::new(),
            knowledge: HashMap::new(),
        }
    }

    fn spawn_handle(mut stream: TcpStream) -> JoinHandle<()> {
        tokio::spawn(async move {
            tracing::warn!("spawned connection handle");
            let mut thread_state = ConnectionThreadState::new(&mut stream);

            let mut last_idle = Option::<Instant>::None;
            let mut send_queue: Vec<seraphic::socket::Response> = vec![];

            loop {
                tokio::select! {
                    Ok(_) = thread_state.read.readable() => {
                        tracing::warn!("readable");
                        last_idle = None;

                        match TcpPacket::read_from_stream(&mut thread_state.read).await {
                            Ok(msg) => {
                                let req: seraphic::socket::Request = msg.try_into().expect("received response in server");
                                let id = req.id.clone();
                                let msg = ServerRPCWrapper::try_from_rpc_req(req)
                                    .expect("failed to get server rpc request");
                                match msg {
                                    ServerRPCWrapper::Relay(lsp_message) => {
                                        tracing::warn!("got lsp_message from client: {lsp_message:#?}");
                                        // for now will just echo
                                        // let json = serde_json::to_value(&lsp_message.payload).unwrap();
                                        let response = ServerRelayResponse { payload: Some(lsp_message.payload)};
                                        // let response = seraphic::socket::Response::from((Ok(json), id));
                                        send_queue.push(response.into_response(id));
                                    }
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                tracing::warn!("would block");
                                tokio::time::sleep(Duration::from_millis(10)).await;
                                continue;
                            }
                            Err(e) => {
                                panic!("failed to read from rpc client on server side: {e:#?}");
                            }
                        }
                    },

                    Ok(_) = thread_state.write.writable() => {
                        tracing::warn!("writable");
                        if !send_queue.is_empty() {
                            last_idle = None;
                            while let Some(res) = send_queue.pop() {
                                tracing::warn!("server responding: {res:#?}");
                                let packet = TcpPacket::serialize(res);
                                // let res =
                                //     serde_json::to_vec(&msg).expect("failed to serialize relay response");

                                thread_state
                                    .write
                                    .write(&packet)
                                    .await
                                    .expect("failed to write rpc response  on serverside");

                                thread_state
                                    .write
                                    .flush()
                                    .await
                                    .expect("failed to flush serverside stream");
                            }
                        }
                    },

                    else => {
                        match last_idle {
                            Some(instant) => {
                                if instant.elapsed() >= ConnectionThreadState::THREAD_IDLE_TIMEOUT {
                                    tracing::warn!("thread timeout!");
                                    return;
                                }
                            }
                            None => {
                                let now = Instant::now();
                                tracing::warn!("setting idle now: {now:#?}");
                                last_idle = Some(now);
                            }
                        }
                    }
                }
            }
        })
    }
}

impl<'s> Server<'s> {
    pub async fn new(config: Config, addr: impl ToSocketAddrs) -> Self {
        let listener = TcpListener::bind(addr).await.expect("could not bind addr");
        let state = ServerState::from_config(config).await;

        Self {
            listener,
            state: Arc::new(RwLock::new(state)),
            connections: HashMap::new(),
        }
    }

    #[tracing::instrument(name = "server main loop", skip_all)]
    pub async fn main_loop(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::warn!("connected: {addr:#?}");
                    let handle = ConnectionThreadState::spawn_handle(stream);
                    self.connections.insert(addr.to_string(), handle);
                }
                Err(e) => tracing::warn!("couldn't accept connection: {e:?}"),
            }
            self.connections.retain(|_, v| !v.is_finished());
        }
    }
}
