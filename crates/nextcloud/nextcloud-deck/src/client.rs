//! Asynchronous client for the Nextcloud Deck REST API.

use std::any::type_name;

use nextcloud_sdk_core::{
    ApiPath, AuthProvider, DecodeError, Error, Headers, HttpMethod, Request, Response, StatusError,
    Transport,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::board::Board;
use crate::card::Card;
use crate::stack::Stack;

#[cfg(test)]
mod tests;

/// Relative path to Nextcloud's Deck API beneath `/index.php`.
pub const DECK_API_ROOT: &str = "apps/deck/api/v1.0";

/// Header that marks a request as an API call for Nextcloud's security checks.
const OCS_REQUEST_HEADER: &str = "OCS-APIRequest";
/// Content negotiation header to request JSON responses.
const ACCEPT_HEADER: &str = "Accept";
/// Content-Type header identifying the JSON payload format.
const CONTENT_TYPE_HEADER: &str = "Content-Type";
/// MIME type for JSON data.
const APPLICATION_JSON: &str = "application/json";

#[derive(Serialize)]
struct CreateBoardPayload<'a> {
    title: &'a str,
    color: &'a str,
}

#[derive(Serialize)]
struct CreateStackPayload<'a> {
    title: &'a str,
    order: i64,
}

#[derive(Serialize)]
struct CreateCardPayload<'a> {
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    order: i64,
}

/// Client for interacting with Nextcloud's Deck REST API.
///
/// Executes calls through any [`Transport`] and applies credentials via an [`AuthProvider`].
/// Both are held by reference, allowing the client to be freely cloned or borrowed across
/// tasks without runtime dispatch or connection pooling overhead.
///
/// All endpoints interact with `/index.php/apps/deck/api/v1.0/...`.
#[derive(Clone, Copy, Debug)]
pub struct DeckClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<'a, T: Transport, A: AuthProvider> DeckClient<'a, T, A> {
    /// Binds a Deck client to a transport and credential provider.
    pub const fn new(transport: &'a T, auth: &'a A) -> Self {
        Self { transport, auth }
    }

    /// The transport this client sends requests through.
    pub const fn transport(&self) -> &'a T {
        self.transport
    }

    /// The credential provider applied to every call.
    pub const fn auth(&self) -> &'a A {
        self.auth
    }

    /// Lists all boards accessible to the authenticated user.
    ///
    /// Sends `GET /index.php/apps/deck/api/v1.0/boards`.
    pub async fn list_boards(&self) -> Result<Vec<Board>, Error> {
        let api_path = ApiPath::public(format!("{DECK_API_ROOT}/boards"))?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        Self::decode_json(HttpMethod::Get, api_path, response)
    }

    /// Retrieves a single board by its `board_id`.
    ///
    /// Sends `GET /index.php/apps/deck/api/v1.0/boards/{board_id}`.
    pub async fn get_board(&self, board_id: u64) -> Result<Board, Error> {
        let api_path = ApiPath::public(format!("{DECK_API_ROOT}/boards/{board_id}"))?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        Self::decode_json(HttpMethod::Get, api_path, response)
    }

    /// Creates a new board with the given `title` and `color`.
    ///
    /// Sends `POST /index.php/apps/deck/api/v1.0/boards` with `{"title": "...", "color": "..."}`.
    pub async fn create_board(&self, title: &str, color: &str) -> Result<Board, Error> {
        let api_path = ApiPath::public(format!("{DECK_API_ROOT}/boards"))?;
        let headers = self.authenticated_headers();
        let payload = CreateBoardPayload { title, color };
        let body = serde_json::to_vec(&payload)
            .map_err(|cause| DecodeError::new(type_name::<CreateBoardPayload<'a>>(), cause))
            .map_err(Error::Decode)?;

        let request = Request::post(api_path.clone())
            .with_headers(headers)
            .body_bytes(body);

        let response = self.transport.execute(request).await?;
        Self::decode_json(HttpMethod::Post, api_path, response)
    }

    /// Deletes a board by its `board_id`.
    ///
    /// Sends `DELETE /index.php/apps/deck/api/v1.0/boards/{board_id}`.
    pub async fn delete_board(&self, board_id: u64) -> Result<(), Error> {
        let api_path = ApiPath::public(format!("{DECK_API_ROOT}/boards/{board_id}"))?;
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        Self::decode_empty(HttpMethod::Delete, api_path, response)
    }

    /// Lists all stacks belonging to a board.
    ///
    /// Sends `GET /index.php/apps/deck/api/v1.0/boards/{board_id}/stacks`.
    pub async fn list_stacks(&self, board_id: u64) -> Result<Vec<Stack>, Error> {
        let api_path = ApiPath::public(format!("{DECK_API_ROOT}/boards/{board_id}/stacks"))?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        Self::decode_json(HttpMethod::Get, api_path, response)
    }

    /// Creates a new stack within a board.
    ///
    /// Sends `POST /index.php/apps/deck/api/v1.0/boards/{board_id}/stacks` with `{"title": "...", "order": ...}`.
    pub async fn create_stack(
        &self,
        board_id: u64,
        title: &str,
        order: i64,
    ) -> Result<Stack, Error> {
        let api_path = ApiPath::public(format!("{DECK_API_ROOT}/boards/{board_id}/stacks"))?;
        let headers = self.authenticated_headers();
        let payload = CreateStackPayload { title, order };
        let body = serde_json::to_vec(&payload)
            .map_err(|cause| DecodeError::new(type_name::<CreateStackPayload<'a>>(), cause))
            .map_err(Error::Decode)?;

        let request = Request::post(api_path.clone())
            .with_headers(headers)
            .body_bytes(body);

        let response = self.transport.execute(request).await?;
        Self::decode_json(HttpMethod::Post, api_path, response)
    }

    /// Deletes a stack from a board.
    ///
    /// Sends `DELETE /index.php/apps/deck/api/v1.0/boards/{board_id}/stacks/{stack_id}`.
    pub async fn delete_stack(&self, board_id: u64, stack_id: u64) -> Result<(), Error> {
        let api_path = ApiPath::public(format!(
            "{DECK_API_ROOT}/boards/{board_id}/stacks/{stack_id}"
        ))?;
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        Self::decode_empty(HttpMethod::Delete, api_path, response)
    }

    /// Lists all cards contained in a stack.
    ///
    /// Sends `GET /index.php/apps/deck/api/v1.0/boards/{board_id}/stacks/{stack_id}/cards`.
    pub async fn list_cards(&self, board_id: u64, stack_id: u64) -> Result<Vec<Card>, Error> {
        let api_path = ApiPath::public(format!(
            "{DECK_API_ROOT}/boards/{board_id}/stacks/{stack_id}/cards"
        ))?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        Self::decode_json(HttpMethod::Get, api_path, response)
    }

    /// Creates a new card in a stack.
    ///
    /// Sends `POST /index.php/apps/deck/api/v1.0/boards/{board_id}/stacks/{stack_id}/cards` with
    /// `{"title": "...", "description": "...", "order": ...}`.
    pub async fn create_card(
        &self,
        board_id: u64,
        stack_id: u64,
        title: &str,
        description: Option<&str>,
        order: i64,
    ) -> Result<Card, Error> {
        let api_path = ApiPath::public(format!(
            "{DECK_API_ROOT}/boards/{board_id}/stacks/{stack_id}/cards"
        ))?;
        let headers = self.authenticated_headers();
        let payload = CreateCardPayload {
            title,
            description,
            order,
        };
        let body = serde_json::to_vec(&payload)
            .map_err(|cause| DecodeError::new(type_name::<CreateCardPayload<'a>>(), cause))
            .map_err(Error::Decode)?;

        let request = Request::post(api_path.clone())
            .with_headers(headers)
            .body_bytes(body);

        let response = self.transport.execute(request).await?;
        Self::decode_json(HttpMethod::Post, api_path, response)
    }

    /// Deletes a card from a stack.
    ///
    /// Sends `DELETE /index.php/apps/deck/api/v1.0/boards/{board_id}/stacks/{stack_id}/cards/{card_id}`.
    pub async fn delete_card(
        &self,
        board_id: u64,
        stack_id: u64,
        card_id: u64,
    ) -> Result<(), Error> {
        let api_path = ApiPath::public(format!(
            "{DECK_API_ROOT}/boards/{board_id}/stacks/{stack_id}/cards/{card_id}"
        ))?;
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        Self::decode_empty(HttpMethod::Delete, api_path, response)
    }

    fn authenticated_headers(&self) -> Headers {
        let mut headers = Headers::new();
        headers.set(OCS_REQUEST_HEADER, "true");
        headers.set(ACCEPT_HEADER, APPLICATION_JSON);
        headers.set(CONTENT_TYPE_HEADER, APPLICATION_JSON);
        self.auth.authenticate(&mut headers);
        headers
    }

    fn decode_json<D: DeserializeOwned>(
        method: HttpMethod,
        path: ApiPath,
        response: Response,
    ) -> Result<D, Error> {
        if !response.status().is_success() {
            return Err(Error::Status(StatusError::new(
                method.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        serde_json::from_slice(response.body())
            .map_err(|cause| DecodeError::new(type_name::<D>(), cause))
            .map_err(Error::Decode)
    }

    fn decode_empty(method: HttpMethod, path: ApiPath, response: Response) -> Result<(), Error> {
        if !response.status().is_success() {
            return Err(Error::Status(StatusError::new(
                method.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }
        Ok(())
    }
}
