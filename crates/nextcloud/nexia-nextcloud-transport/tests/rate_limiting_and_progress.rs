use std::sync::{Arc, Mutex};
use std::time::Instant;

use nexia_nextcloud_core::{ApiPath, HttpMethod, Request, ServerUrl, Transport};
use nexia_nextcloud_transport::{BandwidthLimit, TransportBuilder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn transport_builder_configures_and_dynamically_modifies_limits() {
    let server = ServerUrl::parse("https://cloud.example.org").unwrap();
    let transport = TransportBuilder::with_server(server)
        .upload_limit(BandwidthLimit::from_mib(1))
        .download_limit(BandwidthLimit::from_mib(5))
        .build()
        .unwrap();

    assert_eq!(transport.upload_limit(), BandwidthLimit::from_mib(1));
    assert_eq!(transport.download_limit(), BandwidthLimit::from_mib(5));

    // Dynamically change without rebuild
    transport.set_upload_limit(BandwidthLimit::from_kib(500));
    transport.set_download_limit(BandwidthLimit::unlimited());

    assert_eq!(transport.upload_limit(), BandwidthLimit::from_kib(500));
    assert!(transport.download_limit().is_unlimited());
}

#[tokio::test]
async fn download_reports_progress_and_respects_rate_limit() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 1024];
        let _ = socket.read(&mut buf).await.unwrap();

        let payload = vec![b'A'; 64 * 1024]; // 64 KiB
        let response_headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
            payload.len()
        );

        socket.write_all(response_headers.as_bytes()).await.unwrap();
        // Send in two 32 KiB chunks
        socket.write_all(&payload[..32 * 1024]).await.unwrap();
        socket.write_all(&payload[32 * 1024..]).await.unwrap();
        socket.flush().await.unwrap();
    });

    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = progress_events.clone();

    let server = ServerUrl::parse(format!("http://{addr}")).unwrap();
    let transport = TransportBuilder::with_server(server)
        .download_progress(move |progress| {
            events_clone.lock().unwrap().push(progress);
        })
        .build()
        .unwrap();

    let path = ApiPath::public("test/download").unwrap();
    let request = Request::new(HttpMethod::Get, path);

    let start = Instant::now();
    let response = transport.execute(request).await.unwrap();
    let _ = start.elapsed();

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.body().len(), 64 * 1024);

    let events = progress_events.lock().unwrap().clone();
    assert!(!events.is_empty(), "Expected progress events to be fired");

    // First event should be 0 bytes
    assert_eq!(events[0].transferred_bytes(), 0);
    assert_eq!(events[0].total_bytes(), Some(64 * 1024));

    // Last event should be 100% complete
    let last = events.last().unwrap();
    assert_eq!(last.transferred_bytes(), 64 * 1024);
    assert!(last.is_complete());
    assert_eq!(last.percentage(), Some(100.0));

    server_task.await.unwrap();
}

#[tokio::test]
async fn upload_reports_progress_and_paces_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 128 * 1024];
        let mut total_read = 0;

        // Read until we have read the body
        while total_read < 64 * 1024 {
            let n = socket.read(&mut buf[total_read..]).await.unwrap();
            if n == 0 {
                break;
            }
            total_read += n;
        }

        let resp = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        socket.write_all(resp.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
        total_read
    });

    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = progress_events.clone();

    let server = ServerUrl::parse(format!("http://{addr}")).unwrap();
    let transport = TransportBuilder::with_server(server)
        .upload_progress(move |progress| {
            events_clone.lock().unwrap().push(progress);
        })
        .build()
        .unwrap();

    let path = ApiPath::public("test/upload").unwrap();
    let payload = vec![b'B'; 64 * 1024]; // 64 KiB
    let request = Request::new(HttpMethod::Post, path).body_bytes(payload);

    let response = transport.execute(request).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);

    let events = progress_events.lock().unwrap().clone();
    assert!(!events.is_empty(), "Expected upload progress events");
    assert_eq!(events[0].transferred_bytes(), 0);

    let last = events.last().unwrap();
    assert_eq!(last.transferred_bytes(), 64 * 1024);
    assert!(last.is_complete());

    let server_bytes = server_task.await.unwrap();
    assert!(server_bytes > 64 * 1024); // HTTP headers + 64 KiB body
}
