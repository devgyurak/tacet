//! 센서↔서버 통신(설계 §5.1, AGENTS.md §2 규칙 10·12).
//!
//! 시험장 내부망 전용이다. mDNS로 서버를 찾고, 서버 화면의 페어링 코드로 센서를 등록하고, 자체 서명
//! 인증서를 고정한 TLS로 통신한다. net은 core 타입을 나를 뿐 판정하지 않는다.

pub mod client;
pub mod discovery;
pub mod frame;
pub mod pairing;
pub mod protocol;
pub mod server;
pub mod tls;

pub use client::{SensorLink, ViewerLink, connect_sensor, connect_viewer};
pub use pairing::Fingerprint;
pub use protocol::{Rejection, SessionInfo, Uplink};
pub use server::{ServerEvent, ServerHandle, ServerSettings, serve};
pub use tls::ServerIdentity;

/// 통신 오류.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    /// 소켓 오류.
    #[error("네트워크 오류: {0}")]
    Io(#[from] std::io::Error),
    /// 시간 초과.
    #[error("응답 시간 초과")]
    Timeout,
    /// 서버가 거부했다.
    #[error("서버가 연결을 거부했다: {0:?}")]
    Rejected(Rejection),
    /// 형식이 틀린 메시지.
    #[error("형식이 틀린 메시지: {0}")]
    Malformed(String),
    /// 너무 긴 메시지.
    #[error("메시지가 너무 길다")]
    FrameTooLong,
    /// 연결이 닫혔다.
    #[error("연결이 닫혔다")]
    Closed,
    /// 고정한 서버 인증서와 다르다.
    #[error("서버 인증서가 고정한 값과 다르다 — 다른 서버이거나 중간자일 수 있다")]
    PinMismatch,
    /// TLS 설정 오류.
    #[error("TLS 설정 오류: {0}")]
    Tls(String),
}
