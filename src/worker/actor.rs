use std::fmt;

use nexia_nextcloud_auth::{BasicAuth, SecretString};
use nexia_nextcloud_core::{ApiPath, Error, HttpMethod, Request, ServerUrl, Transport};
use nexia_nextcloud_ocs::OcsClient;
use nexia_nextcloud_sdk_session::{
    Account, LoginFlow, LoginStart, LoginSuccess, PollToken, SessionError, SessionEvent,
    SessionMachine, SessionState,
};
use nexia_nextcloud_transport::ReqwestTransport;
use nexia_nextcloud_users::UserClient;
use tokio::sync::{broadcast, mpsc, watch};

use crate::error::ClientError;
use crate::event::ClientEvent;
use crate::storage::{CacheStore, ConfigStore, SessionStore};
use crate::worker::task::WorkerTask;

#[derive(serde::Deserialize)]
struct LoginInitWire {
    poll: PollWire,
    login: String,
}

#[derive(serde::Deserialize)]
struct PollWire {
    token: String,
    endpoint: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PollSuccessWire {
    server: Option<String>,
    login_name: String,
    app_password: String,
}

/// Persistent database stores bundled for the background worker actor.
pub struct WorkerStores {
    /// Session credential store.
    pub session: SessionStore,
    /// Cached capabilities and metadata store.
    pub cache: CacheStore,
    /// Configuration, theming, and sync mount store.
    pub config: ConfigStore,
}

impl WorkerStores {
    /// Creates a new bundle of storage adapters.
    pub const fn new(session: SessionStore, cache: CacheStore, config: ConfigStore) -> Self {
        Self {
            session,
            cache,
            config,
        }
    }
}

/// Background worker actor maintaining state transitions, network tasks, and storage persistence.
pub struct WorkerActor {
    server_url: ServerUrl,
    transport: ReqwestTransport,
    session_machine: SessionMachine,
    session_store: SessionStore,
    cache_store: CacheStore,
    config_store: ConfigStore,
    state_tx: watch::Sender<SessionState>,
    task_rx: mpsc::Receiver<WorkerTask>,
    event_tx: broadcast::Sender<ClientEvent>,
}

impl WorkerActor {
    /// Creates a new worker actor.
    pub fn new(
        server_url: ServerUrl,
        transport: ReqwestTransport,
        session_machine: SessionMachine,
        stores: WorkerStores,
        state_tx: watch::Sender<SessionState>,
        task_rx: mpsc::Receiver<WorkerTask>,
        event_tx: broadcast::Sender<ClientEvent>,
    ) -> Self {
        Self {
            server_url,
            transport,
            session_machine,
            session_store: stores.session,
            cache_store: stores.cache,
            config_store: stores.config,
            state_tx,
            task_rx,
            event_tx,
        }
    }

    /// Event loop processing worker tasks until all client handles are dropped.
    pub async fn run(mut self) {
        tracing::debug!("Worker actor started event loop");
        while let Some(task) = self.task_rx.recv().await {
            tracing::trace!(task = ?task, "Worker actor received task");
            self.handle_task(task).await;
        }
    }

    async fn handle_task(&mut self, task: WorkerTask) {
        match task {
            WorkerTask::StartLogin { reply } => {
                let res = self.start_login().await;
                let _ = reply.send(res);
            }
            WorkerTask::PollLogin { reply } => {
                let res = self.poll_login().await;
                let _ = reply.send(res);
            }
            WorkerTask::Logout { reply } => {
                let res = self.logout();
                let _ = reply.send(res);
            }
            WorkerTask::StoreCapabilities {
                user_id,
                body,
                reply,
            } => {
                let res = self
                    .cache_store
                    .store_capabilities(&self.server_url, &user_id, &body)
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::LoadCapabilities { reply } => {
                let res = self
                    .cache_store
                    .load_capabilities(&self.server_url)
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::ClearCache { reply } => {
                let res = self.cache_store.clear().map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::GetSessionState { reply } => {
                let _ = reply.send(self.session_machine.state().clone());
            }
            WorkerTask::StoreTheme { theme, reply } => {
                let res = self
                    .config_store
                    .store_theme(self.server_url.as_str(), &theme)
                    .map_err(ClientError::Storage);
                if let Ok(stored) = &res {
                    let _ = self
                        .event_tx
                        .send(ClientEvent::ThemeUpdated(stored.clone()));
                }
                let _ = reply.send(res.map(|_| ()));
            }
            WorkerTask::LoadTheme { reply } => {
                let res = self
                    .config_store
                    .load_theme(self.server_url.as_str())
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::SetSetting { key, value, reply } => {
                let res = self
                    .config_store
                    .set_setting(&key, &value)
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::GetSetting { key, reply } => {
                let res = self
                    .config_store
                    .get_setting(&key)
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::StoreSyncMount { mount, reply } => {
                let res = self
                    .config_store
                    .store_sync_mount(&mount)
                    .map_err(ClientError::Storage);
                if let Ok(stored) = &res {
                    let _ = self
                        .event_tx
                        .send(ClientEvent::SyncMountAdded(stored.clone()));
                }
                let _ = reply.send(res.map(|_| ()));
            }
            WorkerTask::RemoveSyncMount { id, reply } => {
                let res = self
                    .config_store
                    .remove_sync_mount(&id)
                    .map_err(ClientError::Storage);
                if res.is_ok() {
                    let _ = self.event_tx.send(ClientEvent::SyncMountRemoved(id));
                }
                let _ = reply.send(res);
            }
            WorkerTask::ListSyncMounts { reply } => {
                let res = self
                    .config_store
                    .list_sync_mounts()
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::SetDirectorySort {
                dir_path,
                sort_field,
                sort_order,
                reply,
            } => {
                let res = self
                    .config_store
                    .set_directory_sort(&dir_path, &sort_field, &sort_order)
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
            WorkerTask::GetDirectorySort { dir_path, reply } => {
                let res = self
                    .config_store
                    .get_directory_sort(&dir_path)
                    .map_err(ClientError::Storage);
                let _ = reply.send(res);
            }
        }
    }

    async fn start_login(&mut self) -> Result<LoginStart, ClientError> {
        tracing::info!(server_url = %self.server_url, "Starting Login Flow v2");
        let _ = self
            .session_machine
            .handle(SessionEvent::StartRequested)
            .map_err(ClientError::Session)?;

        let path = ApiPath::public("login/v2").map_err(Error::from)?;
        let request = Request::new(HttpMethod::Post, path.clone());

        let response = match self.transport.execute(request).await {
            Ok(resp) => resp,
            Err(err) => {
                let _ = self.session_machine.handle(SessionEvent::FlowStartFailed);
                self.state_tx
                    .send_replace(self.session_machine.state().clone());
                return Err(ClientError::Core(err));
            }
        };

        if !response.status().is_success() {
            let _ = self.session_machine.handle(SessionEvent::FlowStartFailed);
            self.state_tx
                .send_replace(self.session_machine.state().clone());
            return Err(ClientError::Core(Error::Status(
                nexia_nextcloud_core::StatusError::new(
                    "POST",
                    path,
                    response.status(),
                    response.body(),
                ),
            )));
        }

        let body = response.body();
        let wire: LoginInitWire = serde_json::from_slice(body).map_err(ClientError::Json)?;
        let start = LoginStart::new(
            wire.login,
            wire.poll.endpoint,
            PollToken::new(wire.poll.token),
        );

        self.session_machine
            .handle(SessionEvent::FlowStarted(start.clone()))
            .map_err(ClientError::Session)?;

        if let SessionState::AwaitingLogin(ref flow) = *self.session_machine.state() {
            self.session_store
                .store_login_flow(&self.server_url, flow)
                .map_err(ClientError::Storage)?;
        }

        self.state_tx
            .send_replace(self.session_machine.state().clone());
        tracing::info!(login_url = %start.login_url(), "Login Flow v2 initiated successfully");
        Ok(start)
    }

    async fn poll_login(&mut self) -> Result<SessionState, ClientError> {
        tracing::debug!("Polling Login Flow v2 endpoint");
        if self.session_machine.is_authenticated() {
            return Ok(self.session_machine.state().clone());
        }

        let flow: LoginFlow = match self.session_machine.state() {
            SessionState::AwaitingLogin(f) | SessionState::AwaitingGrant(f) => f.clone(),
            other => {
                return Err(ClientError::Session(SessionError::new(
                    other,
                    &SessionEvent::PollPending,
                )));
            }
        };

        let response = self
            .transport
            .client()
            .post(flow.poll_endpoint())
            .header("Content-Type", "application/x-www-form-urlencoded")
            // The token is server-issued opaque text: encode it so a delimiter
            // character cannot terminate the `token` parameter early.
            .body(format!(
                "token={}",
                nexia_nextcloud_core::url::percent_encode(flow.poll_token().expose())
            ))
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let bytes = resp.bytes().await.map_err(Error::transport)?;
                    let wire: PollSuccessWire =
                        serde_json::from_slice(&bytes).map_err(ClientError::Json)?;
                    let server = wire
                        .server
                        .and_then(|s| ServerUrl::parse(&s).ok())
                        .unwrap_or_else(|| self.server_url.clone());

                    let success = LoginSuccess::new(
                        server,
                        wire.login_name,
                        SecretString::new(wire.app_password),
                    );

                    self.session_machine
                        .handle(SessionEvent::Granted(success))
                        .map_err(ClientError::Session)?;

                    if let Some(mut account) = self.session_machine.account().cloned() {
                        let auth = BasicAuth::new(account.login_name(), account.password().clone());
                        let ocs = OcsClient::new(self.transport.clone(), auth);
                        let user_client = UserClient::new(&ocs);

                        if let Ok(user) = user_client.current_user().await {
                            account = Account::new(
                                account.server().clone(),
                                account.login_name(),
                                user.id(),
                                account.password().clone(),
                            );
                            self.session_machine = SessionMachine::with_state(
                                SessionState::Authenticated(account.clone()),
                            );
                        }

                        self.session_store
                            .store_account(&account)
                            .map_err(ClientError::Storage)?;
                        let _ = self.session_store.delete_login_flow(&self.server_url);
                        tracing::info!(user_id = %account.user_id(), "Login Flow v2 succeeded and authenticated");
                    }

                    self.state_tx
                        .send_replace(self.session_machine.state().clone());
                    Ok(self.session_machine.state().clone())
                } else {
                    tracing::debug!("Login Flow v2 authorization still pending");
                    self.session_machine
                        .handle(SessionEvent::PollPending)
                        .map_err(ClientError::Session)?;
                    self.state_tx
                        .send_replace(self.session_machine.state().clone());
                    Ok(self.session_machine.state().clone())
                }
            }
            Err(err) => {
                tracing::error!(error = %err, "Error polling Login Flow v2");
                Err(ClientError::Core(Error::transport(err)))
            }
        }
    }

    fn logout(&mut self) -> Result<(), ClientError> {
        tracing::info!(server_url = %self.server_url, "Logging out user and wiping session credentials");
        self.session_machine
            .handle(SessionEvent::LogoutRequested)
            .map_err(ClientError::Session)?;

        let _ = self.session_store.delete_account(&self.server_url);
        let _ = self.session_store.delete_login_flow(&self.server_url);
        let _ = self.cache_store.clear();

        self.state_tx.send_replace(SessionState::Idle);
        Ok(())
    }
}

impl fmt::Debug for WorkerActor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkerActor")
            .field("server_url", &self.server_url)
            .field("session_state", self.session_machine.state())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexia_nextcloud_sdk_storage::StoreKey;
    use nexia_nextcloud_transport::TransportBuilder;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "worker-test-{name}-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[tokio::test]
    async fn debug_formats_without_leaking_credentials() {
        let auth_path = temp_path("auth");
        let cache_path = temp_path("cache");
        let config_path = temp_path("config");
        let key = StoreKey::generate().unwrap();

        let session_store = SessionStore::open(&auth_path, &key).unwrap();
        let cache_store = CacheStore::open(&cache_path, &key).unwrap();
        let config_store = ConfigStore::open(&config_path, &key).unwrap();

        let server_url = ServerUrl::parse("https://cloud.example.org").unwrap();
        let transport = TransportBuilder::with_server(server_url.clone())
            .build()
            .unwrap();
        let (state_tx, _state_rx) = watch::channel(SessionState::Idle);
        let (_task_tx, task_rx) = mpsc::channel(1);
        let (event_tx, _event_rx) = broadcast::channel(16);

        let stores = WorkerStores::new(session_store, cache_store, config_store);
        let actor = WorkerActor::new(
            server_url,
            transport,
            SessionMachine::new(),
            stores,
            state_tx,
            task_rx,
            event_tx,
        );

        let rendered = format!("{actor:?}");
        assert!(rendered.contains("WorkerActor"));
        assert!(rendered.contains("https://cloud.example.org"));
        assert!(!rendered.contains("password"));

        std::fs::remove_file(&auth_path).ok();
        std::fs::remove_file(&cache_path).ok();
        std::fs::remove_file(&config_path).ok();
    }
}
