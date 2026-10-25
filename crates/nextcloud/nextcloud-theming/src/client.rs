//! Asynchronous client for Nextcloud theming and server status endpoints.

use nextcloud_ocs::OcsEnvelope;
use nextcloud_sdk_core::{
    ApiPath, AuthProvider, DecodeError, Headers, HttpMethod, Request, StatusError, Transport,
};
use serde::Deserialize;

use crate::custom_branding::CustomBranding;
use crate::effective_branding::EffectiveBranding;
use crate::error::ThemingError;
use crate::server_status::ServerStatusInfo;
use crate::theme_info::ThemeInfo;

const OCS_REQUEST_HEADER: &str = "OCS-APIRequest";
const ACCEPT_HEADER: &str = "Accept";
const ACCEPT_JSON: &str = "application/json";

/// Client for querying Nextcloud server theming, branding, and unauthenticated server status.
#[derive(Clone, Copy, Debug)]
pub struct ThemingClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<'a, T: Transport, A: AuthProvider> ThemingClient<'a, T, A> {
    /// Constructs a new theming client.
    pub const fn new(transport: &'a T, auth: &'a A) -> Self {
        Self { transport, auth }
    }

    /// Access the underlying transport.
    pub const fn transport(&self) -> &'a T {
        self.transport
    }

    /// Access the authentication provider.
    pub const fn auth(&self) -> &'a A {
        self.auth
    }

    /// Fetches server theming metadata, querying `/ocs/v2.php/cloud/capabilities` first
    /// with fallback to `/ocs/v2.php/apps/theming/api/v1/theme`.
    ///
    /// # Errors
    ///
    /// Returns [`ThemingError`] if network transport fails or the server returns an error.
    pub async fn get_theme(&self) -> Result<ThemeInfo, ThemingError> {
        tracing::debug!("Fetching Nextcloud theming metadata");

        // 1. Try querying /ocs/v2.php/cloud/capabilities?format=json (standard Nextcloud capabilities)
        if let Ok(caps_path) = ApiPath::ocs("cloud/capabilities?format=json") {
            let mut headers = Headers::new();
            headers.set(OCS_REQUEST_HEADER, "true");
            headers.set(ACCEPT_HEADER, ACCEPT_JSON);
            self.auth.authenticate(&mut headers);

            let request = Request::get(caps_path).with_headers(headers);
            if let Ok(response) = self.transport.execute(request).await
                && response.status().is_success()
            {
                #[derive(Deserialize)]
                struct CapabilitiesThemingNode {
                    #[serde(default)]
                    theming: Option<ThemeInfo>,
                }

                #[derive(Deserialize)]
                struct CapabilitiesContainer {
                    capabilities: CapabilitiesThemingNode,
                }

                if let Ok(envelope) =
                    serde_json::from_slice::<OcsEnvelope<CapabilitiesContainer>>(response.body())
                    && envelope.meta().is_ok()
                    && let Some(theme) = envelope.into_data().capabilities.theming
                {
                    return Ok(theme);
                }
            }
        }

        // 2. Fall back to /ocs/v2.php/apps/theming/api/v1/theme?format=json
        let api_path = ApiPath::ocs("apps/theming/api/v1/theme?format=json")?;

        let mut headers = Headers::new();
        headers.set(OCS_REQUEST_HEADER, "true");
        headers.set(ACCEPT_HEADER, ACCEPT_JSON);
        self.auth.authenticate(&mut headers);

        let request = Request::get(api_path.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        if !response.status().is_success() {
            return Err(ThemingError::Status(StatusError::new(
                HttpMethod::Get.as_str(),
                api_path,
                response.status(),
                response.body(),
            )));
        }

        let envelope: OcsEnvelope<ThemeInfo> = serde_json::from_slice(response.body())
            .map_err(|err| DecodeError::new(std::any::type_name::<ThemeInfo>(), err))?;

        if !envelope.meta().is_ok() {
            return Err(ThemingError::Ocs {
                status_code: envelope.meta().code().unwrap_or(0) as u32,
                message: envelope.meta().message().to_string(),
            });
        }

        Ok(envelope.into_data())
    }

    /// Fetches unauthenticated server status via `GET /status.php`.
    ///
    /// This endpoint does not require user credentials and is safe to call before login.
    ///
    /// # Errors
    ///
    /// Returns [`ThemingError`] if the server is unreachable or responds with non-success.
    pub async fn server_status(&self) -> Result<ServerStatusInfo, ThemingError> {
        tracing::debug!("Querying unauthenticated Nextcloud server status");
        let api_path = ApiPath::raw("status.php")?;

        let mut headers = Headers::new();
        headers.set(ACCEPT_HEADER, ACCEPT_JSON);

        let request = Request::get(api_path.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        if !response.status().is_success() {
            return Err(ThemingError::Status(StatusError::new(
                HttpMethod::Get.as_str(),
                api_path,
                response.status(),
                response.body(),
            )));
        }

        let info: ServerStatusInfo = serde_json::from_slice(response.body())
            .map_err(|err| DecodeError::new(std::any::type_name::<ServerStatusInfo>(), err))?;

        Ok(info)
    }

    /// Computes the effective branding for the application, fetching theme info and server status.
    ///
    /// Falls back gracefully if theming app is disabled or server status is unreachable.
    pub async fn effective_branding(
        &self,
        custom: &CustomBranding,
    ) -> Result<EffectiveBranding, ThemingError> {
        let theme = self.get_theme().await.ok();
        let status = self.server_status().await.ok();
        Ok(custom.resolve(theme.as_ref(), status.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextcloud_auth::Anonymous;
    use nextcloud_sdk_core::{Error as SdkError, Response, StatusCode};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct MockTransport {
        call_count: AtomicUsize,
        theme_json: String,
        status_json: String,
    }

    impl Transport for MockTransport {
        async fn execute(&self, request: Request) -> Result<Response, SdkError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let path = request.path().as_str();

            if path.contains("cloud/capabilities") {
                if self.theme_json.is_empty() {
                    Ok(Response::new(
                        StatusCode::new(404),
                        Headers::new(),
                        Vec::new(),
                    ))
                } else {
                    Ok(Response::new(
                        StatusCode::new(200),
                        Headers::new(),
                        self.theme_json.as_bytes().to_vec(),
                    ))
                }
            } else if path.contains("apps/theming/api/v1/theme") {
                Ok(Response::new(
                    StatusCode::new(200),
                    Headers::new(),
                    self.theme_json.as_bytes().to_vec(),
                ))
            } else if path.contains("status.php") {
                Ok(Response::new(
                    StatusCode::new(200),
                    Headers::new(),
                    self.status_json.as_bytes().to_vec(),
                ))
            } else {
                Ok(Response::new(
                    StatusCode::new(404),
                    Headers::new(),
                    Vec::new(),
                ))
            }
        }
    }

    #[tokio::test]
    async fn fetch_theme_info_and_server_status() {
        let theme_json = r##"{
            "ocs": {
                "meta": {"status": "ok", "statuscode": 200, "message": "OK"},
                "data": {
                    "name": "Acme Cloud",
                    "url": "https://acme.org",
                    "slogan": "Secure Files",
                    "color": "#0082c9",
                    "logo": "https://acme.org/logo.png"
                }
            }
        }"##;

        let status_json = r##"{
            "installed": true,
            "maintenance": false,
            "needsDbUpgrade": false,
            "version": "30.0.0.1",
            "versionstring": "30.0.0",
            "edition": "Enterprise",
            "productname": "Nextcloud"
        }"##;

        let transport = MockTransport {
            call_count: AtomicUsize::new(0),
            theme_json: theme_json.to_string(),
            status_json: status_json.to_string(),
        };

        let auth = Anonymous;
        let client = ThemingClient::new(&transport, &auth);

        let theme = client.get_theme().await.unwrap();
        assert_eq!(theme.name(), Some("Acme Cloud"));
        assert_eq!(theme.color(), Some("#0082c9"));
        assert_eq!(theme.logo(), Some("https://acme.org/logo.png"));

        let status = client.server_status().await.unwrap();
        assert!(status.is_installed());
        assert!(!status.is_maintenance());
        assert_eq!(status.version_string(), Some("30.0.0"));
        assert_eq!(status.product_name(), Some("Nextcloud"));

        let custom = CustomBranding::new("Acme Desktop")
            .with_app_version("2.0.0")
            .with_vendor("Acme Corp");

        let effective = client.effective_branding(&custom).await.unwrap();
        assert_eq!(effective.app_name(), "Acme Desktop");
        assert_eq!(effective.instance_name(), "Acme Cloud");
        assert_eq!(effective.primary_color(), "#0082c9");
        assert_eq!(custom.build_user_agent(), "Acme Desktop/2.0.0 (Acme Corp)");
    }

    #[tokio::test]
    async fn fetch_theme_from_capabilities_with_lossy_bool() {
        let caps_json = r##"{
            "ocs": {
                "meta": {"status": "ok", "statuscode": 200, "message": "OK"},
                "data": {
                    "capabilities": {
                        "theming": {
                            "name": "Custom Organization Cloud",
                            "url": "https://custom.org",
                            "slogan": "Privacy First",
                            "color": "#e95420",
                            "color-text": "#ffffff",
                            "logo": "https://custom.org/logo.svg",
                            "background-plain": "",
                            "background-default": "0"
                        }
                    }
                }
            }
        }"##;

        let transport = MockTransport {
            call_count: AtomicUsize::new(0),
            theme_json: caps_json.to_string(),
            status_json: String::new(),
        };

        let auth = Anonymous;
        let client = ThemingClient::new(&transport, &auth);

        let theme = client.get_theme().await.unwrap();
        assert_eq!(theme.name(), Some("Custom Organization Cloud"));
        assert_eq!(theme.color(), Some("#e95420"));
        assert_eq!(theme.color_text(), Some("#ffffff"));
        assert_eq!(theme.logo(), Some("https://custom.org/logo.svg"));
        assert!(!theme.is_background_plain());
        assert!(!theme.is_background_default());
    }
}
