use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::*;

fn state(code: &str) -> (watch::Sender<BuildingView>, DashboardState) {
    let (tx, rx) = watch::channel(BuildingView {
        phase: tacet_core::view::phase_view(tacet_core::engine::Phase::Watching, 0),
        ..BuildingView::default()
    });
    (tx, DashboardState::new(rx, code.to_owned()))
}

async fn send(state: DashboardState, req: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let response = router(state).oneshot(req).await.expect("응답");
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map(|b| b.to_bytes())
        .unwrap_or_default();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

async fn get_with(
    state: DashboardState,
    uri: &str,
    cookie: Option<&str>,
) -> (StatusCode, HeaderMap, String) {
    let mut req = Request::builder().uri(uri);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    send(state, req.body(Body::empty()).expect("요청")).await
}

async fn login_with(state: DashboardState, code: &str) -> (StatusCode, HeaderMap, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("code={code}")))
        .expect("요청");
    send(state, req).await
}

/// 로그인해서 받은 세션 쿠키(`이름=값`).
async fn session(state: &DashboardState, code: &str) -> String {
    let (_, headers, _) = login_with(state.clone(), code).await;
    let set = headers
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    set.split(';').next().unwrap_or_default().to_owned()
}

#[test]
fn given_new_access_codes_when_generated_then_ten_unambiguous_characters() {
    let code = new_access_code();
    assert_eq!(
        (code.len(), code.bytes().all(|b| CODE_ALPHABET.contains(&b))),
        (CODE_LEN, true)
    );
}

#[test]
fn given_strings_when_compared_in_constant_time_then_equal_only_when_identical() {
    assert_eq!(
        [
            same("AB23", "AB23"),
            same("AB23", "AB24"),
            same("AB2", "AB23")
        ],
        [true, false, false]
    );
}

#[tokio::test]
async fn given_no_session_when_index_opened_then_post_login_form_shown() {
    let (_tx, s) = state("ABCDEFGH23");
    let (status, _, body) = get_with(s, "/", None).await;
    assert_eq!(
        (
            status,
            body.contains("method=\"post\""),
            body.contains("접속 코드(10자리)")
        ),
        (StatusCode::OK, true, true)
    );
}

#[tokio::test]
async fn given_correct_code_when_logged_in_then_random_session_cookie_not_the_code() {
    let (_tx, s) = state("ABCDEFGH23");
    let (status, headers, _) = login_with(s, "abcdefgh23").await;
    let cookie = headers
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        (
            status,
            cookie.starts_with("tacet_session="),
            cookie.contains("ABCDEFGH23"),
            cookie.contains("HttpOnly")
        ),
        (StatusCode::SEE_OTHER, true, false, true)
    );
}

#[tokio::test]
async fn given_wrong_code_when_logged_in_then_unauthorized_with_notice() {
    let (_tx, s) = state("ABCDEFGH23");
    let (status, _, body) = login_with(s, "WRONGWRONG").await;
    assert_eq!(
        (status, body.contains("맞지 않습니다")),
        (StatusCode::UNAUTHORIZED, true)
    );
}

#[tokio::test]
async fn given_five_failures_when_correct_code_tried_then_locked_out() {
    let (_tx, s) = state("ABCDEFGH23");
    let failures: Vec<StatusCode> =
        futures::future::join_all((0..5).map(|_| login_with(s.clone(), "WRONGWRONG")))
            .await
            .into_iter()
            .map(|r| r.0)
            .collect();
    let (status, _, body) = login_with(s, "ABCDEFGH23").await;
    assert_eq!(
        (failures, status, body.contains("잠시 잠갔습니다")),
        (
            vec![StatusCode::UNAUTHORIZED; 5],
            StatusCode::TOO_MANY_REQUESTS,
            true
        )
    );
}

#[tokio::test]
async fn given_session_when_index_and_api_read_then_dashboard_and_view_json() {
    let (tx, s) = state("ABCDEFGH23");
    tx.send_replace(BuildingView {
        phase: tacet_core::view::phase_view(tacet_core::engine::Phase::Baseline, 5_000),
        ..BuildingView::default()
    });
    let cookie = session(&s, "ABCDEFGH23").await;
    let (_, _, page) = get_with(s.clone(), "/", Some(&cookie)).await;
    let (status, _, body) = get_with(s, "/api/view", Some(&format!("other=1; {cookie}"))).await;
    let view: BuildingView = serde_json::from_str(&body).expect("화면 JSON");
    assert_eq!(
        (
            page.contains("<script src=\"app.js\"></script>"),
            status,
            view.phase.remaining_secs
        ),
        (true, StatusCode::OK, 5)
    );
}

#[tokio::test]
async fn given_code_as_cookie_or_no_cookie_when_api_or_events_read_then_unauthorized() {
    let (_tx, s) = state("ABCDEFGH23");
    let api = get_with(s.clone(), "/api/view", None).await.0;
    let events = get_with(s, "/events", Some("tacet_session=ABCDEFGH23"))
        .await
        .0;
    assert_eq!(
        (api, events),
        (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED)
    );
}

#[tokio::test]
async fn given_empty_access_code_when_empty_code_posted_then_still_unauthorized() {
    let (_tx, s) = state("");
    assert_eq!(login_with(s, "").await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn given_session_when_events_opened_then_first_event_carries_view() {
    let (_tx, s) = state("ABCDEFGH23");
    let cookie = session(&s, "ABCDEFGH23").await;
    let req = Request::builder()
        .uri("/events")
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .expect("요청");
    let response = router(s).oneshot(req).await.expect("응답");
    let mut body = response.into_body();
    let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
        .await
        .ok()
        .flatten()
        .and_then(Result::ok);
    let text = frame
        .and_then(|f| f.into_data().ok())
        .map(|d| String::from_utf8_lossy(&d).into_owned())
        .unwrap_or_default();
    assert!(text.starts_with("data: "));
}

#[tokio::test]
async fn given_server_started_on_loopback_when_index_requested_then_login_page_served() {
    let (_tx, s) = state("ABCDEFGH23");
    let (addr, task) = serve("127.0.0.1:0".parse().expect("주소"), s)
        .await
        .expect("시작");
    let mut stream = tokio::net::TcpStream::connect(addr).await.expect("연결");
    tokio::io::AsyncWriteExt::write_all(
        &mut stream,
        b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    )
    .await
    .expect("쓰기");
    let mut buf = Vec::new();
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut buf),
    )
    .await;
    task.abort();
    assert!(String::from_utf8_lossy(&buf).contains("감독관 대시보드"));
}

#[tokio::test]
async fn given_no_session_when_renderer_files_requested_then_served_with_types_and_no_view_data() {
    let (_tx, s) = state("ABCDEFGH23");
    let (app_status, app_headers, app) = get_with(s.clone(), "/app.js", None).await;
    let (css_status, css_headers, _) = get_with(s, "/styles.css", None).await;
    let content_type = |h: &HeaderMap| {
        h.get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    assert_eq!(
        (
            app_status,
            content_type(&app_headers).starts_with("text/javascript"),
            app.contains("최종 확인은 감독관이 합니다"),
            css_status,
            content_type(&css_headers).starts_with("text/css")
        ),
        (StatusCode::OK, true, true, StatusCode::OK, true)
    );
}
