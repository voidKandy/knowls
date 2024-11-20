use std::{
    process::{Child, ChildStdout, Command, Stdio},
    sync::mpsc::{self, TryRecvError},
    thread::JoinHandle,
};

use super::error::DatabaseResult;
use crate::config::database::DatabaseConfig;
use anyhow::anyhow;
use surrealdb::{
    engine::local::{Db, RocksDb},
    opt::auth::Root,
    Surreal,
};
use tracing::warn;

#[derive(Debug)]
pub struct DatabaseThread {
    child_handle: JoinHandle<Child>,
    pub stdout: ChildStdout,
    pub client: Surreal<Db>,
}

enum DatabaseThreadMessage {
    Opened,
    Info { stdout: ChildStdout },
}

const LOCALHOST: &str = "127.0.0.1";
impl DatabaseThread {
    /// Tries to initialize child process handle, if a host is passed, returns None
    pub(super) async fn try_init(
        config: DatabaseConfig,
        database_path: String,
    ) -> DatabaseResult<Self> {
        let (sender, recv) = mpsc::channel::<DatabaseThreadMessage>();

        let address = format!("{LOCALHOST}:{}", config.port);
        let database_path_arg = format!("rocksdb://{database_path}");
        // let (user, pass) = (config.user.to_owned(), config.pass.to_owned());
        warn!("spawning database thread handle..");
        let handle = std::thread::spawn(move || {
            sender
                .send(DatabaseThreadMessage::Opened)
                // .await
                .expect("failed to send");
            warn!("Spinning up database in child process with address: {address}");
            // warn!("Spinning up database in child process with address: {address}");
            let mut child = Command::new("surreal")
                .args([
                    "start",
                    "--log",
                    "debug",
                    // "--no-banner",
                    // "--user",
                    // &user,
                    // "--pass",
                    // &pass,
                    "--bind",
                    &address,
                    &database_path_arg,
                ])
                .stdout(Stdio::piped())
                // .output()
                .spawn()
                .expect("Failed to run database start command");
            let stdout = child.stdout.take().expect("Could not take child stdout");
            sender
                .send(DatabaseThreadMessage::Info { stdout })
                .expect("Could not send child stdout and client");
            child
        });
        warn!("spawned database instance thread");

        let client = Self::init_client(&config, &database_path).await;
        loop {
            match recv.try_recv() {
                Ok(msg) => match msg {
                    DatabaseThreadMessage::Opened => {
                        warn!("Recieved Opened, waiting to get stdout handle");
                        continue;
                    }
                    DatabaseThreadMessage::Info { stdout } => {
                        warn!("recieved stdout handle, returning DatabaseThread");
                        return Ok(Self {
                            child_handle: handle,
                            stdout,
                            client,
                        });
                    }
                },
                Err(err) => match err {
                    TryRecvError::Empty => {
                        // warn!("channel is still open, waiting for stdout...");
                        continue;
                    }
                    TryRecvError::Disconnected => {
                        let e = String::from("channel disconnected before returning stdout");
                        warn!(e);
                        return Err(anyhow!(e).into());
                    }
                },
            }
        }
    }

    async fn init_client(config: &DatabaseConfig, path: &str) -> Surreal<Db> {
        let default_user = Root {
            username: &config.user,
            password: &config.pass,
        };
        warn!("initializing db client with default user {default_user:#?}");
        let db_config = surrealdb::opt::Config::new().user(default_user);

        let client = Surreal::new::<RocksDb>((path, db_config))
            .await
            .expect("failed to create client");

        client.signin(default_user).await.expect("failed sign in");
        warn!("successfully signed client in");

        client
            .use_ns(config.namespace.as_str())
            .use_db(config.database.as_str())
            .await
            .expect("failed to use database or namespace");

        warn!("client initialization successful");
        client
    }

    pub(super) async fn kill(self) -> Result<(), std::io::Error> {
        self.child_handle.join().unwrap().kill()?;
        Ok(())
    }
}
