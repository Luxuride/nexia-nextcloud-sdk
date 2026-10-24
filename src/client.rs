//! High-level, stateful Nextcloud client handle.

use std::fmt;

use nextcloud_activity::ActivityClient;
use nextcloud_auth::BasicAuth;
use nextcloud_dav::DavClient;
use nextcloud_notifications::NotificationClient;
use nextcloud_ocs::OcsClient;
use nextcloud_preview::PreviewClient;
use nextcloud_shares::ShareClient;
use nextcloud_transport::ReqwestTransport;
use nextcloud_trashbin::TrashClient;
use nextcloud_users::UserClient;
use nextcloud_versions::VersionsClient;
use sdk_core::ServerUrl;
use sdk_session::{LoginStart, SessionState};
use sdk_vfs::VirtualTree;
use tokio::sync::{mpsc, oneshot, watch};

use crate::builder::ClientBuilder;
use crate::error::ClientError;
use crate::worker::WorkerTask;

/// A stateful, cloneable handle to the Nextcloud client.
///
/// Communicates with an internal background actor driving the session lifecycle
/// state machine ([`sdk_session::SessionMachine`]), persistent credential storage,
/// and local encrypted cache.
#[derive(Clone)]
pub struct Client {
    server_url: ServerUrl,
    transport: ReqwestTransport,
    task_tx: mpsc::Sender<WorkerTask>,
    state_rx: watch::Receiver<SessionState>,
}

impl Client {
    /// Creates a builder to configure and instantiate a [`Client`].
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub(crate) fn new(
        server_url: ServerUrl,
        transport: ReqwestTransport,
        task_tx: mpsc::Sender<WorkerTask>,
        state_rx: watch::Receiver<SessionState>,
    ) -> Self {
        Self {
            server_url,
            transport,
            task_tx,
            state_rx,
        }
    }

    /// The base URL of the Nextcloud instance.
    pub const fn server_url(&self) -> &ServerUrl {
        &self.server_url
    }

    /// The HTTP transport used by this client.
    pub const fn transport(&self) -> &ReqwestTransport {
        &self.transport
    }

    /// Current session lifecycle state.
    pub fn session_state(&self) -> SessionState {
        self.state_rx.borrow().clone()
    }

    /// Whether the client holds active authenticated credentials.
    pub fn is_authenticated(&self) -> bool {
        self.state_rx.borrow().is_authenticated()
    }

    /// The authenticated user ID, if an active session is established.
    pub fn current_user_id(&self) -> Option<String> {
        self.state_rx
            .borrow()
            .account()
            .map(|a| a.user_id().to_owned())
    }

    /// Initiates Nextcloud Login Flow v2.
    ///
    /// Requests a login flow from the server, persists the pending flow to local storage,
    /// and returns the [`LoginStart`] containing the browser URL and poll endpoint.
    pub async fn start_login(&self) -> Result<LoginStart, ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::StartLogin { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Polls the server for confirmation of the pending login flow.
    ///
    /// Returns the updated [`SessionState`]. If the user has granted access in the browser,
    /// the state becomes [`SessionState::Authenticated`] and credentials are saved to encrypted storage.
    pub async fn poll_login(&self) -> Result<SessionState, ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::PollLogin { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Logs out of the current session, wiping local encrypted credentials and cached records.
    pub async fn logout(&self) -> Result<(), ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::Logout { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Clears cached server capabilities and metadata without logging out.
    pub async fn clear_cache(&self) -> Result<(), ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::ClearCache { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Returns the current [`BasicAuth`] credentials if authenticated.
    pub fn credentials(&self) -> Option<BasicAuth> {
        self.state_rx
            .borrow()
            .account()
            .map(|account| BasicAuth::new(account.login_name(), account.password().clone()))
    }

    /// Constructs an [`OcsClient`] using current credentials.
    pub fn ocs(&self) -> Result<OcsClient<ReqwestTransport, BasicAuth>, ClientError> {
        let auth = self.credentials().ok_or(ClientError::NotAuthenticated)?;
        Ok(OcsClient::new(self.transport.clone(), auth))
    }

    /// Constructs a [`UserClient`] borrowing an [`OcsClient`].
    pub fn users<'a>(
        &'a self,
        ocs: &'a OcsClient<ReqwestTransport, BasicAuth>,
    ) -> UserClient<'a, ReqwestTransport, BasicAuth> {
        UserClient::new(ocs)
    }

    /// Constructs a [`ShareClient`] using current credentials.
    pub fn shares<'a>(
        &'a self,
        auth: &'a BasicAuth,
    ) -> ShareClient<'a, ReqwestTransport, BasicAuth> {
        ShareClient::new(&self.transport, auth)
    }

    /// Constructs a [`NotificationClient`] using current credentials.
    pub fn notifications<'a>(
        &'a self,
        auth: &'a BasicAuth,
    ) -> NotificationClient<'a, ReqwestTransport, BasicAuth> {
        NotificationClient::new(&self.transport, auth)
    }

    /// Constructs an [`ActivityClient`] borrowing an [`OcsClient`].
    pub fn activity<'a>(
        &'a self,
        ocs: &'a OcsClient<ReqwestTransport, BasicAuth>,
    ) -> ActivityClient<'a, ReqwestTransport, BasicAuth> {
        ActivityClient::new(ocs)
    }

    /// Constructs a [`PreviewClient`] using current credentials.
    pub fn preview<'a>(
        &'a self,
        auth: &'a BasicAuth,
    ) -> PreviewClient<'a, ReqwestTransport, BasicAuth> {
        PreviewClient::new(&self.transport, auth)
    }

    /// Constructs a [`DavClient`] using current credentials.
    pub fn dav<'a>(&'a self, auth: &'a BasicAuth) -> DavClient<'a, ReqwestTransport, BasicAuth> {
        DavClient::new(&self.transport, auth)
    }

    /// Constructs a [`TrashClient`] using current credentials.
    pub fn trashbin<'a>(
        &'a self,
        auth: &'a BasicAuth,
    ) -> TrashClient<'a, ReqwestTransport, BasicAuth> {
        TrashClient::new(&self.transport, auth)
    }

    /// Constructs a [`VersionsClient`] using current credentials.
    pub fn versions<'a>(
        &'a self,
        auth: &'a BasicAuth,
    ) -> VersionsClient<'a, ReqwestTransport, BasicAuth> {
        VersionsClient::new(&self.transport, auth)
    }

    /// Instantiates an empty, transient [`VirtualTree`].
    pub fn new_vfs(root_name: impl Into<String>) -> VirtualTree {
        VirtualTree::new(root_name)
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("server_url", &self.server_url)
            .field("session_state", &*self.state_rx.borrow())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextcloud_transport::TransportBuilder;
    use sdk_core::ServerUrl;

    #[tokio::test]
    async fn client_accessors_and_debug_format() {
        let server_url = ServerUrl::parse("https://cloud.example.org").unwrap();
        let transport = TransportBuilder::with_server(server_url.clone())
            .build()
            .unwrap();
        let (_state_tx, state_rx) = watch::channel(SessionState::Idle);
        let (task_tx, _task_rx) = mpsc::channel(1);

        let client = Client::new(server_url.clone(), transport, task_tx, state_rx);

        assert_eq!(client.server_url(), &server_url);
        assert_eq!(client.session_state(), SessionState::Idle);
        assert!(!client.is_authenticated());
        assert_eq!(client.current_user_id(), None);
        assert!(client.credentials().is_none());
        assert!(matches!(client.ocs(), Err(ClientError::NotAuthenticated)));

        let auth = BasicAuth::new("alice", nextcloud_auth::SecretString::new("secret"));
        let _dav = client.dav(&auth);
        let _trash = client.trashbin(&auth);
        let _versions = client.versions(&auth);
        let _shares = client.shares(&auth);
        let _notifications = client.notifications(&auth);
        let _preview = client.preview(&auth);

        let vfs = Client::new_vfs("root");
        assert_eq!(vfs.len(), 1);

        let rendered = format!("{client:?}");
        assert!(rendered.contains("Client"));
        assert!(rendered.contains("https://cloud.example.org"));
    }
}
