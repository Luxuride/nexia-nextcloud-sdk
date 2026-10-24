use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use nextcloud_auth::SecretString;
use nextcloud_transport::{ReqwestTransport, TimeoutConfig, TransportBuilder};
use sdk_core::ServerUrl;
use sdk_request::RetryPolicy;
use sdk_session::{Account, SessionMachine, SessionState};
use sdk_storage::{KeyStore, KeychainKeyStore, StorageError, StoragePaths, StoreKey};
use tokio::sync::{mpsc, watch};

use crate::client::Client;
use crate::error::ClientError;
use crate::storage::{CacheStore, SessionStore};
use crate::worker::WorkerActor;

type KeyLoader = Box<
    dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<StoreKey, StorageError>> + Send>> + Send,
>;

enum StoreKeySource {
    Direct(StoreKey),
    Loader(KeyLoader),
}

/// Builder for configuring and establishing a stateful [`Client`].
pub struct ClientBuilder {
    server_url: Option<ServerUrl>,
    storage_paths: Option<StoragePaths>,
    store_key: Option<StoreKeySource>,
    initial_account: Option<Account>,
    initial_credentials: Option<(String, String, SecretString)>,
    retry_policy: Option<RetryPolicy>,
    timeout_config: Option<TimeoutConfig>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    /// Creates a blank builder.
    pub fn new() -> Self {
        Self {
            server_url: None,
            storage_paths: None,
            store_key: None,
            initial_account: None,
            initial_credentials: None,
            retry_policy: None,
            timeout_config: None,
        }
    }

    /// Sets the Nextcloud server instance base URL. (Required)
    pub fn server_url(mut self, server_url: ServerUrl) -> Self {
        self.server_url = Some(server_url);
        self
    }

    /// Sets the discrete database file paths for credentials and cache. (Required)
    pub fn storage_paths(
        mut self,
        auth_db_path: impl Into<PathBuf>,
        cache_db_path: impl Into<PathBuf>,
    ) -> Self {
        self.storage_paths = Some(StoragePaths::new(auth_db_path, cache_db_path));
        self
    }

    /// Sets storage database paths using [`StoragePaths`].
    pub fn storage(mut self, paths: StoragePaths) -> Self {
        self.storage_paths = Some(paths);
        self
    }

    /// Configures an explicit 256-bit database encryption key.
    pub fn store_key(mut self, key: StoreKey) -> Self {
        self.store_key = Some(StoreKeySource::Direct(key));
        self
    }

    /// Configures an asynchronous [`KeyStore`] to load or generate the database encryption key.
    pub fn key_store<K: KeyStore + 'static>(mut self, store: K) -> Self {
        self.store_key = Some(StoreKeySource::Loader(Box::new(move || {
            Box::pin(async move { store.load_or_create().await })
        })));
        self
    }

    /// Configures Linux Secret Service storage via `oo7` using [`KeychainKeyStore`].
    pub fn keychain(mut self, app_id: impl Into<String>, account: impl Into<String>) -> Self {
        let app_id = app_id.into();
        let account = account.into();
        self.store_key = Some(StoreKeySource::Loader(Box::new(move || {
            Box::pin(async move {
                let ks = KeychainKeyStore::new(app_id, account);
                ks.load_or_create().await
            })
        })));
        self
    }

    /// Supplies an already-known authenticated [`Account`].
    pub fn initial_account(mut self, account: Account) -> Self {
        self.initial_account = Some(account);
        self
    }

    /// Supplies initial login name and app password credentials.
    pub fn initial_credentials(
        mut self,
        login_name: impl Into<String>,
        app_password: SecretString,
    ) -> Self {
        let name = login_name.into();
        self.initial_credentials = Some((name.clone(), name, app_password));
        self
    }

    /// Sets the retry policy for request operations.
    pub fn retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = Some(retry_policy);
        self
    }

    /// Configures transport timeouts.
    pub fn timeout_config(mut self, timeout_config: TimeoutConfig) -> Self {
        self.timeout_config = Some(timeout_config);
        self
    }

    /// Finalizes configuration, opens the encrypted databases, starts the worker actor, and returns a [`Client`].
    pub async fn build(self) -> Result<Client, ClientError> {
        let server_url = self
            .server_url
            .ok_or(ClientError::MissingConfiguration("server_url"))?;

        let storage_paths = self
            .storage_paths
            .ok_or(ClientError::MissingConfiguration("storage_paths"))?;

        let key_source = self
            .store_key
            .ok_or(ClientError::MissingConfiguration("store_key"))?;

        let key = match key_source {
            StoreKeySource::Direct(k) => k,
            StoreKeySource::Loader(loader) => loader().await.map_err(ClientError::Storage)?,
        };

        let session_store = SessionStore::open(storage_paths.auth(), &key)?;
        let cache_store = CacheStore::open(storage_paths.cache(), &key)?;

        let session_machine = if let Some(account) = self.initial_account {
            session_store.store_account(&account)?;
            SessionMachine::with_state(SessionState::Authenticated(account))
        } else if let Some((login_name, user_id, pw)) = self.initial_credentials {
            let account = Account::new(server_url.clone(), login_name, user_id, pw);
            session_store.store_account(&account)?;
            SessionMachine::with_state(SessionState::Authenticated(account))
        } else if let Some(account) = session_store.load_account(&server_url)? {
            SessionMachine::with_state(SessionState::Authenticated(account))
        } else if let Some(flow) = session_store.load_login_flow(&server_url)? {
            SessionMachine::with_state(SessionState::AwaitingLogin(flow))
        } else {
            SessionMachine::new()
        };

        let mut transport_builder = TransportBuilder::with_server(server_url.clone());
        if let Some(timeouts) = self.timeout_config {
            transport_builder = transport_builder.timeouts(timeouts);
        }
        let transport: ReqwestTransport = transport_builder.build()?;

        let initial_state = session_machine.state().clone();
        let (task_tx, task_rx) = mpsc::channel(32);
        let (state_tx, state_rx) = watch::channel(initial_state);

        let actor = WorkerActor::new(
            server_url.clone(),
            transport.clone(),
            session_machine,
            session_store,
            cache_store,
            state_tx,
            task_rx,
        );

        tokio::spawn(async move {
            actor.run().await;
        });

        Ok(Client::new(server_url, transport, task_tx, state_rx))
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("server_url", &self.server_url)
            .field("storage_paths", &self.storage_paths)
            .field("has_store_key", &self.store_key.is_some())
            .field("has_initial_account", &self.initial_account.is_some())
            .field(
                "has_initial_credentials",
                &self.initial_credentials.is_some(),
            )
            .field("retry_policy", &self.retry_policy)
            .field("timeout_config", &self.timeout_config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "client-builder-{name}-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[tokio::test]
    async fn builder_requires_server_url() {
        let auth = temp_path("req_auth");
        let cache = temp_path("req_cache");
        let key = StoreKey::generate().unwrap();

        let res = ClientBuilder::new()
            .storage_paths(&auth, &cache)
            .store_key(key)
            .build()
            .await;

        assert!(matches!(
            res,
            Err(ClientError::MissingConfiguration("server_url"))
        ));
    }

    #[tokio::test]
    async fn builder_requires_storage_paths() {
        let server_url = ServerUrl::parse("https://cloud.example.org").unwrap();
        let key = StoreKey::generate().unwrap();

        let res = ClientBuilder::new()
            .server_url(server_url)
            .store_key(key)
            .build()
            .await;

        assert!(matches!(
            res,
            Err(ClientError::MissingConfiguration("storage_paths"))
        ));
    }

    #[tokio::test]
    async fn builder_requires_store_key() {
        let server_url = ServerUrl::parse("https://cloud.example.org").unwrap();
        let auth = temp_path("req_auth2");
        let cache = temp_path("req_cache2");

        let res = ClientBuilder::new()
            .server_url(server_url)
            .storage_paths(&auth, &cache)
            .build()
            .await;

        assert!(matches!(
            res,
            Err(ClientError::MissingConfiguration("store_key"))
        ));
    }

    #[tokio::test]
    async fn builder_success_with_initial_credentials() {
        let server_url = ServerUrl::parse("https://cloud.example.org").unwrap();
        let auth = temp_path("succ_auth");
        let cache = temp_path("succ_cache");
        let key = StoreKey::generate().unwrap();

        let client = ClientBuilder::new()
            .server_url(server_url.clone())
            .storage_paths(&auth, &cache)
            .store_key(key)
            .initial_credentials("alice", SecretString::new("secret-pass"))
            .build()
            .await
            .expect("build client");

        assert_eq!(client.server_url(), &server_url);
        assert!(client.is_authenticated());
        assert_eq!(client.current_user_id().as_deref(), Some("alice"));

        std::fs::remove_file(&auth).ok();
        std::fs::remove_file(&cache).ok();
    }
}
