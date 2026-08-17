use super::support::TestServer;

/// A browser will not send a management command at all unless the preflight is
/// answered, and will not hand the response back to the page without the
/// allow-origin header, so the web build depends on both.
#[tokio::test]
async fn management_answers_a_cors_preflight() {
    let server = TestServer::start().await;
    let url = format!("{}/api/management", server.url);

    let preflight = tokio::task::spawn_blocking({
        let url = url.clone();
        move || {
            ureq::request("OPTIONS", &url)
                .set("origin", "https://blocks.example.com")
                .set("access-control-request-method", "POST")
                .set("access-control-request-headers", "content-type")
                .call()
                .expect("the preflight must be answered")
        }
    })
    .await
    .unwrap();

    assert_eq!(preflight.status(), 204);
    assert_eq!(preflight.header("access-control-allow-origin"), Some("*"));
    assert!(preflight
        .header("access-control-allow-methods")
        .is_some_and(|methods| methods.contains("POST")));
    assert!(preflight
        .header("access-control-allow-headers")
        .is_some_and(|headers| headers.contains("content-type")));

    // The command's own response needs the header too; the preflight only
    // grants permission to send it.
    let response = tokio::task::spawn_blocking(move || {
        match ureq::post(&url)
            .set("content-type", "application/json")
            .send_bytes(b"not json")
        {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(error) => panic!("management request failed: {error}"),
        }
    })
    .await
    .unwrap();
    assert_eq!(response.header("access-control-allow-origin"), Some("*"));

    server.cleanup().await;
}
