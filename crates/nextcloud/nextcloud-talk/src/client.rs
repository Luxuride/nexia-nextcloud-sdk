//! Asynchronous client for Nextcloud Talk (Spreed) API.
//!
//! Official documentation:
//! - [Nextcloud Talk API documentation](https://nextcloud-talk.readthedocs.io/en/latest/)

use nexia_nextcloud_core::{ApiPath, AuthProvider, Error, Transport};
use nextcloud_ocs::{OcsClient, OcsFormat, Query};
use serde::Serialize;

use crate::message::ChatMessage;
use crate::room::Room;
use crate::room_type::RoomType;

/// Relative path to Nextcloud Talk room API beneath `/ocs/v2.php`.
pub const ROOM_API_PATH: &str = "apps/spreed/api/v4/room";

/// Relative path to Nextcloud Talk chat API beneath `/ocs/v2.php`.
pub const CHAT_API_PATH: &str = "apps/spreed/api/v1/chat";

/// Asynchronous client for Nextcloud Talk (Spreed) API.
///
/// Wraps an [`OcsClient`] by reference and provides high-level typed calls for:
/// - Listing conversations / rooms (`/ocs/v2.php/apps/spreed/api/v4/room`)
/// - Fetching conversation details by token (`/ocs/v2.php/apps/spreed/api/v4/room/{token}`)
/// - Creating conversations (`/ocs/v2.php/apps/spreed/api/v4/room`)
/// - Deleting conversations (`/ocs/v2.php/apps/spreed/api/v4/room/{token}`)
/// - Listing chat messages (`/ocs/v2.php/apps/spreed/api/v1/chat/{token}`)
/// - Sending chat messages (`/ocs/v2.php/apps/spreed/api/v1/chat/{token}`)
#[derive(Clone, Copy, Debug)]
pub struct TalkClient<'a, T: Transport, A: AuthProvider> {
    ocs: &'a OcsClient<T, A>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateRoomRequest<'a> {
    room_type: RoomType,
    #[serde(skip_serializing_if = "Option::is_none")]
    invite: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_name: Option<&'a str>,
}

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    message: &'a str,
}

impl<'a, T: Transport, A: AuthProvider> TalkClient<'a, T, A> {
    /// Binds a talk client to an underlying [`OcsClient`].
    pub const fn new(ocs: &'a OcsClient<T, A>) -> Self {
        Self { ocs }
    }

    /// Borrows the underlying [`OcsClient`].
    pub const fn ocs(&self) -> &'a OcsClient<T, A> {
        self.ocs
    }

    /// Lists all conversations the authenticated user belongs to.
    ///
    /// Sends `GET /ocs/v2.php/apps/spreed/api/v4/room`.
    pub async fn list_rooms(&self) -> Result<Vec<Room>, Error> {
        tracing::debug!("Listing Talk rooms");
        let path = ApiPath::ocs(ROOM_API_PATH)?;
        self.ocs
            .get(path, Query::new().format(OcsFormat::Json))
            .await
    }

    /// Retrieves conversation details for a specific room token.
    ///
    /// Sends `GET /ocs/v2.php/apps/spreed/api/v4/room/{token}`.
    pub async fn get_room(&self, token: &str) -> Result<Room, Error> {
        tracing::debug!(token, "Fetching Talk room details");
        let path = ApiPath::ocs(format!("{ROOM_API_PATH}/{token}"))?;
        self.ocs
            .get(path, Query::new().format(OcsFormat::Json))
            .await
    }

    /// Creates a new conversation room.
    ///
    /// Sends `POST /ocs/v2.php/apps/spreed/api/v4/room`.
    pub async fn create_room(
        &self,
        room_type: RoomType,
        invite: Option<&str>,
        name: Option<&str>,
    ) -> Result<Room, Error> {
        tracing::info!(?room_type, ?name, "Creating Talk room");
        let path = ApiPath::ocs(ROOM_API_PATH)?;
        let request = CreateRoomRequest {
            room_type,
            invite,
            room_name: name,
        };
        self.ocs
            .post_with_query(path, Query::new().format(OcsFormat::Json), &request)
            .await
    }

    /// Deletes a conversation room by its token.
    ///
    /// Sends `DELETE /ocs/v2.php/apps/spreed/api/v4/room/{token}`.
    pub async fn delete_room(&self, token: &str) -> Result<(), Error> {
        tracing::info!(token, "Deleting Talk room");
        let path = ApiPath::ocs(format!("{ROOM_API_PATH}/{token}"))?;
        self.ocs
            .delete_with_query::<()>(path, Query::new().format(OcsFormat::Json))
            .await
    }

    /// Lists chat messages in a conversation.
    ///
    /// Sends `GET /ocs/v2.php/apps/spreed/api/v1/chat/{token}` with `lookIntoFuture=0`
    /// and optional `limit`.
    pub async fn list_messages(
        &self,
        token: &str,
        limit: Option<u32>,
    ) -> Result<Vec<ChatMessage>, Error> {
        let path = ApiPath::ocs(format!("{CHAT_API_PATH}/{token}"))?;
        let mut query = Query::new()
            .format(OcsFormat::Json)
            .param("lookIntoFuture", "0");
        if let Some(limit) = limit {
            query = query.param("limit", limit.to_string());
        }
        self.ocs.get(path, query).await
    }

    /// Sends a new chat message to a conversation.
    ///
    /// Sends `POST /ocs/v2.php/apps/spreed/api/v1/chat/{token}` with `{"message": "..."}`.
    pub async fn send_message(&self, token: &str, message: &str) -> Result<ChatMessage, Error> {
        let path = ApiPath::ocs(format!("{CHAT_API_PATH}/{token}"))?;
        let request = SendMessageRequest { message };
        self.ocs
            .post_with_query(path, Query::new().format(OcsFormat::Json), &request)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    use nexia_nextcloud_core::{
        AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
    };
    use nextcloud_ocs::OcsClient;

    use super::TalkClient;
    use crate::room_type::RoomType;

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[derive(Clone, Debug)]
    struct FakeTransport {
        response: Response,
        seen: Arc<Mutex<Option<Request>>>,
    }

    impl FakeTransport {
        fn new(status: u16, body: &str) -> Self {
            Self {
                response: Response::new(StatusCode::new(status), Headers::new(), body.as_bytes()),
                seen: Arc::new(Mutex::new(None)),
            }
        }

        fn request(&self) -> Option<Request> {
            self.seen.lock().ok().and_then(|s| s.clone())
        }
    }

    impl Transport for FakeTransport {
        async fn execute(&self, request: Request) -> Result<Response, Error> {
            if let Ok(mut seen) = self.seen.lock() {
                *seen = Some(request);
            }
            Ok(self.response.clone())
        }
    }

    #[derive(Clone, Debug)]
    struct FakeAuth;

    impl AuthProvider for FakeAuth {
        fn authenticate(&self, headers: &mut Headers) {
            headers.set("Authorization", "Bearer test-talk-token");
        }
    }

    const LIST_ROOMS_RESPONSE: &str = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": [
                {
                    "token": "tok1",
                    "name": "General",
                    "displayName": "General",
                    "type": 2,
                    "description": "General discussions",
                    "hasPassword": false,
                    "participantCount": 8
                },
                {
                    "token": "tok2",
                    "name": "Direct Alice",
                    "type": 1,
                    "hasPassword": false
                }
            ]
        }
    }"#;

    #[test]
    fn test_list_rooms() {
        let transport = FakeTransport::new(200, LIST_ROOMS_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = TalkClient::new(&ocs);

        let rooms = block_on(client.list_rooms()).expect("list rooms succeeds");
        assert_eq!(rooms.len(), 2);
        assert_eq!(rooms[0].token, "tok1");
        assert_eq!(rooms[0].name, "General");
        assert_eq!(rooms[0].room_type, RoomType::Group);
        assert_eq!(rooms[0].description.as_deref(), Some("General discussions"));
        assert!(!rooms[0].has_password);
        assert_eq!(rooms[0].participant_count, Some(8));

        assert_eq!(rooms[1].token, "tok2");
        assert_eq!(rooms[1].room_type, RoomType::OneToOne);

        let req = transport.request().expect("request was sent");
        assert_eq!(req.method(), HttpMethod::Get);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/spreed/api/v4/room?format=json"
        );
    }

    const GET_ROOM_RESPONSE: &str = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": {
                "token": "abcde",
                "roomName": "Dev Team",
                "roomType": 2,
                "description": "Engineers only",
                "hasPassword": true,
                "participantCount": 4
            }
        }
    }"#;

    #[test]
    fn test_get_room() {
        let transport = FakeTransport::new(200, GET_ROOM_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = TalkClient::new(&ocs);

        let room = block_on(client.get_room("abcde")).expect("get room succeeds");
        assert_eq!(room.token, "abcde");
        assert_eq!(room.name, "Dev Team");
        assert_eq!(room.room_type, RoomType::Group);
        assert_eq!(room.description.as_deref(), Some("Engineers only"));
        assert!(room.has_password);
        assert_eq!(room.participant_count, Some(4));

        let req = transport.request().expect("request was sent");
        assert_eq!(req.method(), HttpMethod::Get);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/spreed/api/v4/room/abcde?format=json"
        );
    }

    const CREATE_ROOM_RESPONSE: &str = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": {
                "token": "newtok123",
                "name": "New Project",
                "type": 2,
                "description": "",
                "hasPassword": false
            }
        }
    }"#;

    #[test]
    fn test_create_room() {
        let transport = FakeTransport::new(200, CREATE_ROOM_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = TalkClient::new(&ocs);

        let room = block_on(client.create_room(RoomType::Group, None, Some("New Project")))
            .expect("create room succeeds");
        assert_eq!(room.token, "newtok123");
        assert_eq!(room.name, "New Project");
        assert_eq!(room.room_type, RoomType::Group);

        let req = transport.request().expect("request was sent");
        assert_eq!(req.method(), HttpMethod::Post);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/spreed/api/v4/room?format=json"
        );

        let body_str = String::from_utf8(req.body().expect("request body").to_vec()).unwrap();
        assert!(body_str.contains(r#""roomType":2"#));
        assert!(body_str.contains(r#""roomName":"New Project""#));
        assert!(!body_str.contains("invite"));
    }

    const EMPTY_OCS_RESPONSE: &str = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": []
        }
    }"#;

    #[test]
    fn test_delete_room() {
        let transport = FakeTransport::new(200, EMPTY_OCS_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = TalkClient::new(&ocs);

        block_on(client.delete_room("deadbeef")).expect("delete room succeeds");

        let req = transport.request().expect("request was sent");
        assert_eq!(req.method(), HttpMethod::Delete);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/spreed/api/v4/room/deadbeef?format=json"
        );
    }

    const LIST_MESSAGES_RESPONSE: &str = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": [
                {
                    "id": 1,
                    "token": "tok1",
                    "actorType": "users",
                    "actorId": "bob",
                    "actorDisplayName": "Bob",
                    "timestamp": 1718000000,
                    "message": "Hi all",
                    "systemMessage": ""
                },
                {
                    "id": 2,
                    "token": "tok1",
                    "actorType": "users",
                    "actorId": "system",
                    "actorDisplayName": "System",
                    "timestamp": 1718000100,
                    "message": "User left",
                    "systemMessage": "user_left"
                }
            ]
        }
    }"#;

    #[test]
    fn test_list_messages() {
        let transport = FakeTransport::new(200, LIST_MESSAGES_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = TalkClient::new(&ocs);

        let messages =
            block_on(client.list_messages("tok1", Some(50))).expect("list messages succeeds");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, 1);
        assert_eq!(messages[0].actor_id, "bob");
        assert_eq!(messages[0].message, "Hi all");
        assert!(!messages[0].is_system_message());

        assert_eq!(messages[1].id, 2);
        assert_eq!(messages[1].system_message, "user_left");
        assert!(messages[1].is_system_message());

        let req = transport.request().expect("request was sent");
        assert_eq!(req.method(), HttpMethod::Get);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/spreed/api/v1/chat/tok1?format=json&lookIntoFuture=0&limit=50"
        );
    }

    const SEND_MESSAGE_RESPONSE: &str = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": {
                "id": 42,
                "token": "tok1",
                "actorType": "users",
                "actorId": "alice",
                "actorDisplayName": "Alice Cooper",
                "timestamp": 1718000200,
                "message": "Hello world!",
                "systemMessage": ""
            }
        }
    }"#;

    #[test]
    fn test_send_message() {
        let transport = FakeTransport::new(200, SEND_MESSAGE_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = TalkClient::new(&ocs);

        let msg =
            block_on(client.send_message("tok1", "Hello world!")).expect("send message succeeds");
        assert_eq!(msg.id, 42);
        assert_eq!(msg.token, "tok1");
        assert_eq!(msg.actor_id, "alice");
        assert_eq!(msg.actor_display_name, "Alice Cooper");
        assert_eq!(msg.message, "Hello world!");

        let req = transport.request().expect("request was sent");
        assert_eq!(req.method(), HttpMethod::Post);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/spreed/api/v1/chat/tok1?format=json"
        );

        let body_str = String::from_utf8(req.body().expect("request body").to_vec()).unwrap();
        assert_eq!(body_str, r#"{"message":"Hello world!"}"#);
    }
}
