//! Integration tests for Nextcloud Talk client.

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nextcloud_ocs::OcsClient;
use nextcloud_sdk_core::{
    AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
};
use nextcloud_talk::{RoomType, TalkClient};

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
struct MockTransport {
    responses: Arc<Mutex<Vec<Response>>>,
    sent_requests: Arc<Mutex<Vec<Request>>>,
}

impl MockTransport {
    fn new(responses: Vec<(u16, &str)>) -> Self {
        let responses = responses
            .into_iter()
            .map(|(status, body)| {
                Response::new(StatusCode::new(status), Headers::new(), body.as_bytes())
            })
            .collect();
        Self {
            responses: Arc::new(Mutex::new(responses)),
            sent_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<Request> {
        self.sent_requests.lock().unwrap().clone()
    }
}

impl Transport for MockTransport {
    async fn execute(&self, request: Request) -> Result<Response, Error> {
        self.sent_requests.lock().unwrap().push(request);
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            panic!("MockTransport ran out of responses");
        }
        Ok(responses.remove(0))
    }
}

#[derive(Clone, Debug)]
struct TestAuth;

impl AuthProvider for TestAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", "Bearer mock-token");
    }
}

#[test]
fn integration_room_lifecycle_and_messaging() {
    let create_room_resp = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": {
                "id": 42,
                "token": "tok_flow",
                "type": 2,
                "name": "Integration Test Room",
                "displayName": "Integration Test Room",
                "description": "Room for tests",
                "hasPassword": false,
                "participantCount": 1
            }
        }
    }"#;

    let get_room_resp = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": {
                "id": 42,
                "token": "tok_flow",
                "type": 2,
                "name": "Integration Test Room",
                "displayName": "Integration Test Room",
                "description": "Room for tests",
                "hasPassword": false,
                "participantCount": 2
            }
        }
    }"#;

    let send_msg_resp = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": {
                "id": 999,
                "token": "tok_flow",
                "actorType": "users",
                "actorId": "test_bot",
                "actorDisplayName": "Test Bot",
                "timestamp": 1720000000,
                "message": "Hello Talk!",
                "systemMessage": ""
            }
        }
    }"#;

    let list_msgs_resp = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": [
                {
                    "id": 999,
                    "token": "tok_flow",
                    "actorType": "users",
                    "actorId": "test_bot",
                    "actorDisplayName": "Test Bot",
                    "timestamp": 1720000000,
                    "message": "Hello Talk!",
                    "systemMessage": ""
                }
            ]
        }
    }"#;

    let delete_room_resp = r#"{
        "ocs": {
            "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
            "data": []
        }
    }"#;

    let transport = MockTransport::new(vec![
        (200, create_room_resp),
        (200, get_room_resp),
        (200, send_msg_resp),
        (200, list_msgs_resp),
        (200, delete_room_resp),
    ]);

    let ocs = OcsClient::new(transport.clone(), TestAuth);
    let client = TalkClient::new(&ocs);

    // 1. Create Room
    let created =
        block_on(client.create_room(RoomType::Group, None, Some("Integration Test Room")))
            .expect("room created");
    assert_eq!(created.token, "tok_flow");
    assert_eq!(created.room_type, RoomType::Group);

    // 2. Get Room
    let fetched = block_on(client.get_room("tok_flow")).expect("room fetched");
    assert_eq!(fetched.token, "tok_flow");
    assert_eq!(fetched.participant_count, Some(2));

    // 3. Send Message
    let sent = block_on(client.send_message("tok_flow", "Hello Talk!")).expect("message sent");
    assert_eq!(sent.id, 999);
    assert_eq!(sent.message, "Hello Talk!");

    // 4. List Messages
    let messages = block_on(client.list_messages("tok_flow", Some(10))).expect("messages listed");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, 999);

    // 5. Delete Room
    block_on(client.delete_room("tok_flow")).expect("room deleted");

    let reqs = transport.requests();
    assert_eq!(reqs.len(), 5);
    assert_eq!(reqs[0].method(), HttpMethod::Post);
    assert_eq!(reqs[1].method(), HttpMethod::Get);
    assert_eq!(reqs[2].method(), HttpMethod::Post);
    assert_eq!(reqs[3].method(), HttpMethod::Get);
    assert_eq!(reqs[4].method(), HttpMethod::Delete);
}
