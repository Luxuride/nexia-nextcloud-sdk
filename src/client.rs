//! High-level, stateful Nextcloud client handle.

use std::fmt;

use nextcloud_activity::ActivityClient;
use nextcloud_auth::BasicAuth;
use nextcloud_dav::DavClient;
use nextcloud_deck::DeckClient;
use nextcloud_notifications::NotificationClient;
use nextcloud_ocs::OcsClient;
use nextcloud_preview::PreviewClient;
use nextcloud_sdk_core::ServerUrl;
use nextcloud_sdk_session::{LoginStart, SessionState};
use nextcloud_sdk_vfs::VirtualTree;
use nextcloud_search::SearchClient;
use nextcloud_shares::ShareClient;
use nextcloud_talk::TalkClient;
use nextcloud_transport::{BandwidthLimit, BandwidthLimiter, ProgressCallback, ReqwestTransport};
use nextcloud_trashbin::TrashClient;
use nextcloud_uploads::UploadClient;
use nextcloud_users::UserClient;
use nextcloud_versions::VersionsClient;
use tokio::sync::{mpsc, oneshot, watch};

use crate::builder::ClientBuilder;
use crate::error::ClientError;
use crate::worker::WorkerTask;

/// A stateful, cloneable handle to the Nextcloud client.
///
/// Communicates with an internal background actor driving the session lifecycle
/// state machine ([`nextcloud_sdk_session::SessionMachine`]), persistent credential storage,
/// and local encrypted cache.
#[derive(Clone)]
pub struct Client {
    server_url: ServerUrl,
    transport: ReqwestTransport,
    task_tx: mpsc::Sender<WorkerTask>,
    state_rx: watch::Receiver<SessionState>,
    branding: Option<nextcloud_theming::CustomBranding>,
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
        branding: Option<nextcloud_theming::CustomBranding>,
    ) -> Self {
        Self {
            server_url,
            transport,
            task_tx,
            state_rx,
            branding,
        }
    }

    /// Client application custom branding metadata, if configured.
    pub fn branding(&self) -> Option<&nextcloud_theming::CustomBranding> {
        self.branding.as_ref()
    }

    /// The base URL of the Nextcloud instance.
    pub const fn server_url(&self) -> &ServerUrl {
        &self.server_url
    }

    /// The HTTP transport used by this client.
    pub const fn transport(&self) -> &ReqwestTransport {
        &self.transport
    }

    /// Reference to the active bandwidth limiter.
    pub const fn limiter(&self) -> &BandwidthLimiter {
        self.transport.limiter()
    }

    /// The currently active upload bandwidth limit.
    pub fn upload_limit(&self) -> BandwidthLimit {
        self.transport.upload_limit()
    }

    /// The currently active download bandwidth limit.
    pub fn download_limit(&self) -> BandwidthLimit {
        self.transport.download_limit()
    }

    /// Dynamically updates the upload bandwidth limit without requiring client rebuild.
    pub fn set_upload_limit(&self, limit: impl Into<BandwidthLimit>) {
        let limit = limit.into();
        tracing::info!(?limit, "Dynamically updating client upload limit");
        self.transport.set_upload_limit(limit);
    }

    /// Dynamically updates the download bandwidth limit without requiring client rebuild.
    pub fn set_download_limit(&self, limit: impl Into<BandwidthLimit>) {
        let limit = limit.into();
        tracing::info!(?limit, "Dynamically updating client download limit");
        self.transport.set_download_limit(limit);
    }

    /// The currently active default download progress callback, if any.
    pub fn download_progress(&self) -> Option<ProgressCallback> {
        self.transport.download_progress()
    }

    /// Dynamically sets or clears the default download progress callback without requiring client rebuild.
    pub fn set_download_progress(&self, callback: Option<ProgressCallback>) {
        self.transport.set_download_progress(callback);
    }

    /// The currently active default upload progress callback, if any.
    pub fn upload_progress(&self) -> Option<ProgressCallback> {
        self.transport.upload_progress()
    }

    /// Dynamically sets or clears the default upload progress callback without requiring client rebuild.
    pub fn set_upload_progress(&self, callback: Option<ProgressCallback>) {
        self.transport.set_upload_progress(callback);
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
        tracing::debug!("Client dispatching start_login task");
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
        tracing::debug!("Client dispatching poll_login task");
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::PollLogin { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Logs out of the current session, wiping local encrypted credentials and cached records.
    pub async fn logout(&self) -> Result<(), ClientError> {
        tracing::info!("Client dispatching logout task");
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::Logout { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Clears cached server capabilities and metadata without logging out.
    pub async fn clear_cache(&self) -> Result<(), ClientError> {
        tracing::info!("Client dispatching clear_cache task");
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

    /// Constructs a [`SearchClient`] borrowing an [`OcsClient`].
    pub fn search<'a>(
        &'a self,
        ocs: &'a OcsClient<ReqwestTransport, BasicAuth>,
    ) -> SearchClient<'a, ReqwestTransport, BasicAuth> {
        SearchClient::new(ocs)
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

    /// Constructs an [`UploadClient`] using current credentials.
    pub fn uploads<'a>(
        &'a self,
        auth: &'a BasicAuth,
    ) -> UploadClient<'a, ReqwestTransport, BasicAuth> {
        UploadClient::new(&self.transport, auth)
    }

    /// Constructs a [`DeckClient`] using current credentials.
    pub fn deck<'a>(&'a self, auth: &'a BasicAuth) -> DeckClient<'a, ReqwestTransport, BasicAuth> {
        DeckClient::new(&self.transport, auth)
    }

    /// Constructs a [`TalkClient`] borrowing an [`OcsClient`].
    pub fn talk<'a>(
        &'a self,
        ocs: &'a OcsClient<ReqwestTransport, BasicAuth>,
    ) -> TalkClient<'a, ReqwestTransport, BasicAuth> {
        TalkClient::new(ocs)
    }

    /// Instantiates an empty, transient [`VirtualTree`].
    pub fn new_vfs(root_name: impl Into<String>) -> VirtualTree {
        VirtualTree::new(root_name)
    }

    /// Constructs a [`ThemingClient`](nextcloud_theming::ThemingClient) using authenticated credentials.
    pub fn theming<'a>(
        &'a self,
        auth: &'a BasicAuth,
    ) -> nextcloud_theming::ThemingClient<'a, ReqwestTransport, BasicAuth> {
        nextcloud_theming::ThemingClient::new(&self.transport, auth)
    }

    /// Constructs an unauthenticated [`ThemingClient`](nextcloud_theming::ThemingClient).
    pub fn anonymous_theming(
        &self,
    ) -> nextcloud_theming::ThemingClient<'_, ReqwestTransport, nextcloud_auth::Anonymous> {
        nextcloud_theming::ThemingClient::new(&self.transport, &nextcloud_auth::Anonymous)
    }

    /// Fetches unauthenticated server status from `/status.php`.
    pub async fn server_status(
        &self,
    ) -> Result<nextcloud_theming::ServerStatusInfo, nextcloud_theming::ThemingError> {
        self.anonymous_theming().server_status().await
    }

    /// Stores or updates visual branding theme metadata in the local permanent config database.
    pub async fn store_cached_theme(
        &self,
        theme: crate::storage::CachedTheme,
    ) -> Result<(), ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::StoreTheme { theme, reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Retrieves visual branding theme metadata from the local permanent config database.
    pub async fn load_cached_theme(
        &self,
    ) -> Result<Option<crate::storage::CachedTheme>, ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::LoadTheme { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Stores a persistent runtime setting in the permanent configuration database.
    pub async fn set_setting(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::SetSetting {
                key: key.into(),
                value: value.into(),
                reply,
            })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Retrieves a persistent runtime setting from the permanent configuration database.
    pub async fn get_setting(&self, key: impl Into<String>) -> Result<Option<String>, ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::GetSetting {
                key: key.into(),
                reply,
            })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Stores or updates a sync mount in the permanent configuration database.
    pub async fn store_sync_mount(
        &self,
        mount: crate::storage::StoredSyncMount,
    ) -> Result<(), ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::StoreSyncMount { mount, reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Removes a sync mount by its unique identifier from the permanent configuration database.
    pub async fn remove_sync_mount(&self, id: impl Into<String>) -> Result<(), ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::RemoveSyncMount {
                id: id.into(),
                reply,
            })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }

    /// Lists all sync mounts stored in the permanent configuration database.
    pub async fn list_sync_mounts(
        &self,
    ) -> Result<Vec<crate::storage::StoredSyncMount>, ClientError> {
        let (reply, rx) = oneshot::channel();
        self.task_tx
            .send(WorkerTask::ListSyncMounts { reply })
            .await
            .map_err(|_| ClientError::WorkerGone)?;
        rx.await.map_err(|_| ClientError::WorkerGone)?
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("server_url", &self.server_url)
            .field("session_state", &*self.state_rx.borrow())
            .field("branding", &self.branding)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextcloud_sdk_core::ServerUrl;
    use nextcloud_transport::TransportBuilder;

    #[tokio::test]
    async fn client_accessors_and_debug_format() {
        let server_url = ServerUrl::parse("https://cloud.example.org").unwrap();
        let transport = TransportBuilder::with_server(server_url.clone())
            .build()
            .unwrap();
        let (_state_tx, state_rx) = watch::channel(SessionState::Idle);
        let (task_tx, _task_rx) = mpsc::channel(1);

        let client = Client::new(server_url.clone(), transport, task_tx, state_rx, None);

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
        let _uploads = client.uploads(&auth);
        let _deck = client.deck(&auth);

        let vfs = Client::new_vfs("root");
        assert_eq!(vfs.len(), 1);

        assert!(client.upload_limit().is_unlimited());
        assert!(client.download_limit().is_unlimited());

        client.set_upload_limit(BandwidthLimit::from_mib(2));
        client.set_download_limit(BandwidthLimit::from_mib(10));
        assert_eq!(client.upload_limit(), BandwidthLimit::from_mib(2));
        assert_eq!(client.download_limit(), BandwidthLimit::from_mib(10));
        assert_eq!(client.limiter().upload_limit(), BandwidthLimit::from_mib(2));

        assert!(client.download_progress().is_none());
        client.set_download_progress(Some(std::sync::Arc::new(|_p| {})));
        assert!(client.download_progress().is_some());

        let rendered = format!("{client:?}");
        assert!(rendered.contains("Client"));
        assert!(rendered.contains("https://cloud.example.org"));
    }
}
