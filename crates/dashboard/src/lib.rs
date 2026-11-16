//! 서버 모드의 감독관 웹 대시보드(설계 §5.0, §5.1).
//!
//! 감독관 태블릿은 앱을 설치하지 않고 이 페이지에 브라우저로 접속한다. 시험장 내부망에만 연다 — 인터넷에
//! 노출하지 않는다(AGENTS.md §3.3). 서버 화면에 보이는 접속 코드를 알아야 볼 수 있다. 화면은 판단하지
//! 않고 core가 만든 [`BuildingView`]를 그릴 뿐이다(AGENTS.md §2 규칙 11).

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Extension, Form, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use futures::Stream;
use rand::Rng;
use serde::Deserialize;
use tacet_core::view::BuildingView;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::WatchStream;

#[cfg(test)]
mod tests;

/// 세션 토큰을 담는 쿠키 이름. 쿠키에는 접속 코드가 아니라 로그인할 때마다 새로 만든 토큰을 담는다.
const COOKIE: &str = "tacet_session";
/// 이만큼 틀리면 그 출처의 로그인을 잠근다.
const MAX_FAILURES: u32 = 5;
/// 잠금 시간.
const LOCKOUT: Duration = Duration::from_secs(300);
/// 접속 코드 글자(헷갈리는 0·O·1·I·L 제외).
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// 접속 코드 길이(31^10 ≈ 8×10^14).
pub const CODE_LEN: usize = 10;

/// 새 접속 코드.
pub fn new_access_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN)
        .map(|_| char::from(CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())]))
        .collect()
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 상수 시간 비교(길이는 드러나도 된다 — 코드 길이는 공개 값이다).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[derive(Default)]
struct Auth {
    tokens: BTreeSet<String>,
    failures: BTreeMap<String, (u32, Instant)>,
}

/// 대시보드 상태. `Debug`는 접속 코드와 세션 토큰을 가린다 — 로그에 찍혀도 새지 않게.
#[derive(Clone)]
pub struct DashboardState {
    view: watch::Receiver<BuildingView>,
    access_code: String,
    auth: Arc<Mutex<Auth>>,
}

impl DashboardState {
    /// 화면 수신기와 접속 코드로 만든다. 코드가 비어 있으면 아무도 들어오지 못한다.
    pub fn new(view: watch::Receiver<BuildingView>, access_code: String) -> Self {
        Self {
            view,
            access_code,
            auth: Arc::new(Mutex::new(Auth::default())),
        }
    }
}

impl std::fmt::Debug for DashboardState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashboardState")
            .field("access_code", &"<가림>")
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct LoginForm {
    code: String,
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|pair| {
            pair.trim()
                .strip_prefix(&format!("{COOKIE}="))
                .map(str::to_owned)
        })
}

fn authorized(state: &DashboardState, headers: &HeaderMap) -> bool {
    let Some(token) = session_token(headers) else {
        return false;
    };
    state
        .auth
        .lock()
        .map(|a| a.tokens.contains(&token))
        .unwrap_or(false)
}

/// 대시보드 라우터.
pub fn router(state: DashboardState) -> Router {
    Router::new()
        .route("/", get(index))
        .route(
            "/app.js",
            get(|| async { asset("text/javascript; charset=utf-8", APP_JS) }),
        )
        .route(
            "/view.js",
            get(|| async { asset("text/javascript; charset=utf-8", VIEW_JS) }),
        )
        .route(
            "/styles.css",
            get(|| async { asset("text/css; charset=utf-8", STYLES) }),
        )
        .route("/login", post(login))
        .route("/api/view", get(api_view))
        .route("/events", get(events))
        .with_state(state)
}

async fn index(State(state): State<DashboardState>, headers: HeaderMap) -> Response {
    if authorized(&state, &headers) {
        Html(PAGE).into_response()
    } else {
        Html(login_page(None)).into_response()
    }
}

async fn login(
    State(state): State<DashboardState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    Form(form): Form<LoginForm>,
) -> Response {
    let source = peer.map_or_else(
        || "알 수 없음".to_owned(),
        |Extension(ConnectInfo(addr))| addr.ip().to_string(),
    );
    let Ok(mut auth) = state.auth.lock() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let now = Instant::now();
    // 잠금이 끝난 출처는 지운다 — 실패 기록이 출처 수만큼 계속 쌓이지 않게.
    auth.failures
        .retain(|_, (_, since)| now.duration_since(*since) < LOCKOUT);
    if let Some((count, since)) = auth.failures.get(&source).copied() {
        if count >= MAX_FAILURES && now.duration_since(since) < LOCKOUT {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Html(login_page(Some(
                    "실패가 많아 잠시 잠갔습니다. 5분 뒤 다시 시도하세요.",
                ))),
            )
                .into_response();
        }
        if now.duration_since(since) >= LOCKOUT {
            auth.failures.remove(&source);
        }
    }
    let code = form.code.trim().to_uppercase();
    if state.access_code.is_empty() || !same(&code, &state.access_code) {
        let entry = auth.failures.entry(source).or_insert((0, now));
        entry.0 += 1;
        entry.1 = now;
        return (
            StatusCode::UNAUTHORIZED,
            Html(login_page(Some("접속 코드가 맞지 않습니다."))),
        )
            .into_response();
    }
    auth.failures.remove(&source);
    let token = new_token();
    auth.tokens.insert(token.clone());
    let cookie = format!("{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/");
    let mut response = Redirect::to("/").into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

async fn api_view(State(state): State<DashboardState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    // 여러 기기가 보는 곳이라 기술 정보(식별자 포함)는 빼고 보낸다(AGENTS.md §3.3).
    let view = state.view.borrow().without_technical();
    axum::Json(view).into_response()
}

fn view_stream(
    view: watch::Receiver<BuildingView>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    WatchStream::new(view).map(|v| {
        // 여러 기기가 보는 곳이라 기술 정보(식별자 포함)는 빼고 보낸다(AGENTS.md §3.3).
        Ok(
            Event::default()
                .data(serde_json::to_string(&v.without_technical()).unwrap_or_default()),
        )
    })
}

async fn events(State(state): State<DashboardState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Sse::new(view_stream(state.view.clone()))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

/// 대시보드를 띄운다. `bind`는 내부망 인터페이스여야 한다.
pub async fn serve(
    bind: SocketAddr,
    state: DashboardState,
) -> std::io::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let addr = listener.local_addr()?;
    let task = tokio::spawn(async move {
        let service = router(state).into_make_service_with_connect_info::<SocketAddr>();
        let _ = axum::serve(listener, service).await;
    });
    Ok((addr, task))
}

fn login_page(notice: Option<&str>) -> String {
    let notice = notice.map_or_else(String::new, |n| format!("<p class=\"err\">{n}</p>"));
    format!(
        "<!doctype html><html lang=\"ko\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>감독관 대시보드</title><style>body{{font-family:-apple-system,'Apple SD Gothic Neo','Malgun Gothic',system-ui,sans-serif;max-width:28rem;margin:4rem auto;padding:0 1rem;color:#1b1f27}}\
input,button{{font-size:1.1rem;padding:.6rem .8rem;border-radius:10px;border:1px solid #d5d9e0}}button{{background:#2f5bea;color:#fff;border-color:#2f5bea;font-weight:600}}.err{{color:#c4282d;font-weight:600}}</style></head><body>\
<h1>감독관 대시보드</h1><p>시험본부 서버 화면에 보이는 접속 코드(10자리)를 입력하세요.</p>{notice}\
<form method=\"post\" action=\"/login\"><input name=\"code\" autocomplete=\"off\" autocapitalize=\"characters\" aria-label=\"접속 코드\"> <button>보기</button></form>\
</body></html>"
    )
}

/// 화면 파일. 데스크톱 앱(Electron, `desktop/renderer/`)과 같은 화면을 쓴다 — 한 곳만 고치면 둘 다 바뀐다. 화면은
/// 판단하지 않고 core가 만든 쉬운 말 모델을 그린다. 파일에는 관측 데이터가 없으므로 로그인 없이 내준다.
const PAGE: &str = include_str!("../../../desktop/renderer/index.html");
const APP_JS: &str = include_str!("../../../desktop/renderer/app.js");
const VIEW_JS: &str = include_str!("../../../desktop/renderer/view.js");
const STYLES: &str = include_str!("../../../desktop/renderer/styles.css");

fn asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}
