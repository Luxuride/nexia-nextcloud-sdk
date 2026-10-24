use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nextcloud_sdk_core::{
    AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
};

use super::DeckClient;

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
        self.seen.lock().ok().and_then(|seen| seen.clone())
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
        headers.set("Authorization", "Bearer deck-test-token");
    }
}

const BOARD_JSON: &str = r#"{
    "id": 1,
    "title": "Project Alpha",
    "color": "0800fd",
    "archived": false,
    "owner": {
        "primaryKey": "alice",
        "uid": "alice",
        "displayname": "Alice Developer"
    },
    "permissions": {
        "PERMISSION_READ": true,
        "PERMISSION_EDIT": true,
        "PERMISSION_WRITE": true,
        "PERMISSION_MANAGE": true,
        "PERMISSION_SHARE": true,
        "PERMISSION_DELETE": false
    },
    "lastModified": 1700000000,
    "deletedAt": 0
}"#;

const BOARDS_LIST_JSON: &str = r#"[
    {
        "id": 1,
        "title": "Project Alpha",
        "color": "0800fd",
        "archived": false,
        "owner": {
            "uid": "alice",
            "displayname": "Alice Developer"
        },
        "permissions": {
            "PERMISSION_READ": true,
            "PERMISSION_EDIT": true
        }
    },
    {
        "id": 2,
        "title": "Sprint 42",
        "color": "ff0000",
        "archived": true,
        "owner": "bob"
    }
]"#;

const STACK_JSON: &str = r#"{
    "id": 10,
    "title": "In Progress",
    "boardId": 1,
    "order": 1,
    "cards": [
        {
            "id": 100,
            "title": "Implement Deck API",
            "description": "Full Rust SDK integration",
            "stackId": 10,
            "boardId": 1,
            "order": 0,
            "archived": false,
            "duedate": "2026-11-01T12:00:00Z",
            "owner": "alice"
        }
    ],
    "deletedAt": 0
}"#;

const STACKS_LIST_JSON: &str = r#"[
    {
        "id": 10,
        "title": "To Do",
        "boardId": 1,
        "order": 0,
        "cards": []
    },
    {
        "id": 11,
        "title": "Done",
        "boardId": 1,
        "order": 1,
        "cards": [
            {
                "id": 101,
                "title": "Design schema",
                "description": "",
                "stackId": 11,
                "order": 0,
                "archived": false
            }
        ]
    }
]"#;

const CARD_JSON: &str = r#"{
    "id": 100,
    "title": "Implement Deck API",
    "description": "Full Rust SDK integration",
    "stackId": 10,
    "boardId": 1,
    "order": 0,
    "archived": false,
    "duedate": "2026-11-01T12:00:00Z",
    "owner": "alice",
    "createdAt": 1699990000,
    "lastModified": 1700000000,
    "deletedAt": 0
}"#;

const CARDS_LIST_JSON: &str = r#"[
    {
        "id": 100,
        "title": "Task 1",
        "description": "First task",
        "stackId": 10,
        "order": 0,
        "archived": false
    },
    {
        "id": 101,
        "title": "Task 2",
        "description": null,
        "stackId": 10,
        "order": 1,
        "archived": false
    }
]"#;

#[test]
fn list_boards_sends_request_and_decodes_response() {
    let transport = FakeTransport::new(200, BOARDS_LIST_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let boards = block_on(client.list_boards()).expect("list boards succeeds");

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Get);
    assert_eq!(req.path().as_str(), "/index.php/apps/deck/api/v1.0/boards");
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(req.headers().get("Content-Type"), Some("application/json"));
    assert_eq!(req.headers().get("Accept"), Some("application/json"));
    assert_eq!(
        req.headers().get("Authorization"),
        Some("Bearer deck-test-token")
    );

    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].id, 1);
    assert_eq!(boards[0].title, "Project Alpha");
    assert_eq!(boards[0].color.as_deref(), Some("0800fd"));
    assert!(!boards[0].archived);
    let owner0 = boards[0].owner.as_ref().unwrap();
    assert_eq!(owner0.uid, "alice");
    assert_eq!(owner0.displayname.as_deref(), Some("Alice Developer"));
    let perms0 = boards[0].permissions.as_ref().unwrap();
    assert!(perms0.read);
    assert!(perms0.edit);

    assert_eq!(boards[1].id, 2);
    assert_eq!(boards[1].title, "Sprint 42");
    assert!(boards[1].archived);
    assert_eq!(boards[1].owner.as_ref().unwrap().uid, "bob");
}

#[test]
fn get_board_sends_request_and_decodes_response() {
    let transport = FakeTransport::new(200, BOARD_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let board = block_on(client.get_board(1)).expect("get board succeeds");

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Get);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/1"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));

    assert_eq!(board.id, 1);
    assert_eq!(board.title, "Project Alpha");
    assert_eq!(board.last_modified, Some(1700000000));
    assert_eq!(board.deleted_at, Some(0));
    let perms = board.permissions.unwrap();
    assert!(perms.read);
    assert!(perms.edit);
    assert!(perms.write);
    assert!(perms.manage);
    assert!(perms.share);
    assert!(!perms.delete);
}

#[test]
fn create_board_sends_json_payload_and_decodes_response() {
    let transport = FakeTransport::new(200, BOARD_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let board =
        block_on(client.create_board("Project Alpha", "0800fd")).expect("create board succeeds");

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Post);
    assert_eq!(req.path().as_str(), "/index.php/apps/deck/api/v1.0/boards");
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(req.headers().get("Content-Type"), Some("application/json"));

    let body_bytes = req.body().expect("request has body");
    let json: serde_json::Value = serde_json::from_slice(body_bytes).expect("valid json body");
    assert_eq!(json["title"], "Project Alpha");
    assert_eq!(json["color"], "0800fd");

    assert_eq!(board.id, 1);
    assert_eq!(board.title, "Project Alpha");
}

#[test]
fn delete_board_sends_delete_request() {
    let transport = FakeTransport::new(200, "");
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let result = block_on(client.delete_board(42));
    assert!(result.is_ok());

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Delete);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/42"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
}

#[test]
fn list_stacks_sends_request_and_decodes_response() {
    let transport = FakeTransport::new(200, STACKS_LIST_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let stacks = block_on(client.list_stacks(1)).expect("list stacks succeeds");

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Get);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/1/stacks"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));

    assert_eq!(stacks.len(), 2);
    assert_eq!(stacks[0].id, 10);
    assert_eq!(stacks[0].title, "To Do");
    assert_eq!(stacks[0].board_id, 1);
    assert_eq!(stacks[0].order, 0);
    assert!(stacks[0].cards.is_empty());

    assert_eq!(stacks[1].id, 11);
    assert_eq!(stacks[1].title, "Done");
    assert_eq!(stacks[1].order, 1);
    assert_eq!(stacks[1].cards.len(), 1);
    assert_eq!(stacks[1].cards[0].id, 101);
}

#[test]
fn create_stack_sends_json_payload_and_decodes_response() {
    let transport = FakeTransport::new(200, STACK_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let stack = block_on(client.create_stack(1, "In Progress", 1)).expect("create stack succeeds");

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Post);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/1/stacks"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(req.headers().get("Content-Type"), Some("application/json"));

    let body_bytes = req.body().expect("request has body");
    let json: serde_json::Value = serde_json::from_slice(body_bytes).expect("valid json body");
    assert_eq!(json["title"], "In Progress");
    assert_eq!(json["order"], 1);

    assert_eq!(stack.id, 10);
    assert_eq!(stack.title, "In Progress");
    assert_eq!(stack.cards.len(), 1);
    assert_eq!(stack.cards[0].id, 100);
}

#[test]
fn delete_stack_sends_delete_request() {
    let transport = FakeTransport::new(200, "");
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let result = block_on(client.delete_stack(1, 10));
    assert!(result.is_ok());

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Delete);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/1/stacks/10"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
}

#[test]
fn list_cards_sends_request_and_decodes_response() {
    let transport = FakeTransport::new(200, CARDS_LIST_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let cards = block_on(client.list_cards(1, 10)).expect("list cards succeeds");

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Get);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/1/stacks/10/cards"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));

    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].id, 100);
    assert_eq!(cards[0].title, "Task 1");
    assert_eq!(cards[0].description, "First task");
    assert_eq!(cards[0].stack_id, 10);
    assert_eq!(cards[0].order, 0);

    assert_eq!(cards[1].id, 101);
    assert_eq!(cards[1].title, "Task 2");
    assert_eq!(cards[1].description, "");
    assert_eq!(cards[1].stack_id, 10);
    assert_eq!(cards[1].order, 1);
}

#[test]
fn create_card_with_description_sends_json_payload_and_decodes_response() {
    let transport = FakeTransport::new(200, CARD_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let card = block_on(client.create_card(
        1,
        10,
        "Implement Deck API",
        Some("Full Rust SDK integration"),
        0,
    ))
    .expect("create card succeeds");

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Post);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/1/stacks/10/cards"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(req.headers().get("Content-Type"), Some("application/json"));

    let body_bytes = req.body().expect("request has body");
    let json: serde_json::Value = serde_json::from_slice(body_bytes).expect("valid json body");
    assert_eq!(json["title"], "Implement Deck API");
    assert_eq!(json["description"], "Full Rust SDK integration");
    assert_eq!(json["order"], 0);

    assert_eq!(card.id, 100);
    assert_eq!(card.title, "Implement Deck API");
    assert_eq!(card.description, "Full Rust SDK integration");
    assert_eq!(card.stack_id, 10);
    assert_eq!(card.board_id, Some(1));
    assert_eq!(card.due_date.as_deref(), Some("2026-11-01T12:00:00Z"));
    assert_eq!(card.owner.as_deref(), Some("alice"));
    assert_eq!(card.created_at, Some(1699990000));
}

#[test]
fn create_card_without_description_omits_description_in_json() {
    let transport = FakeTransport::new(200, CARD_JSON);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let _card = block_on(client.create_card(1, 10, "Task without description", None, 3))
        .expect("create card succeeds");

    let req = transport.request().expect("request was sent");
    let body_bytes = req.body().expect("request has body");
    let json: serde_json::Value = serde_json::from_slice(body_bytes).expect("valid json body");
    assert_eq!(json["title"], "Task without description");
    assert_eq!(json["order"], 3);
    assert!(json.get("description").is_none());
}

#[test]
fn delete_card_sends_delete_request() {
    let transport = FakeTransport::new(200, "");
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let result = block_on(client.delete_card(1, 10, 100));
    assert!(result.is_ok());

    let req = transport.request().expect("request was sent");
    assert_eq!(req.method(), HttpMethod::Delete);
    assert_eq!(
        req.path().as_str(),
        "/index.php/apps/deck/api/v1.0/boards/1/stacks/10/cards/100"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
}

#[test]
fn client_transport_and_auth_accessors() {
    let transport = FakeTransport::new(200, "[]");
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let _t = client.transport();
    let _a = client.auth();
}

#[test]
fn error_on_non_2xx_status() {
    let transport = FakeTransport::new(404, r#"{"status": 404, "message": "Board not found"}"#);
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let err = block_on(client.get_board(999)).unwrap_err();
    match err {
        Error::Status(status) => {
            assert_eq!(status.status().as_u16(), 404);
            assert!(status.body().contains("Board not found"));
        }
        other => panic!("expected Error::Status, got {other:?}"),
    }
}

#[test]
fn error_on_malformed_json_response() {
    let transport = FakeTransport::new(200, "invalid-json-content");
    let auth = FakeAuth;
    let client = DeckClient::new(&transport, &auth);

    let err = block_on(client.list_boards()).unwrap_err();
    match err {
        Error::Decode(decode) => {
            assert!(decode.target().contains("Board"));
        }
        other => panic!("expected Error::Decode, got {other:?}"),
    }
}
