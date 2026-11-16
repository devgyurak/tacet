//! 메시지 형식. 버전이 맞지 않으면 서버가 연결을 거부한다.

use serde::{Deserialize, Serialize};
use tacet_core::obs::{CollectorKind, CollectorStatus, SensorInfo, SensorReport};
use tacet_core::view::BuildingView;

/// 프로토콜 버전.
pub const VERSION: u32 = 1;

/// 접속하는 쪽의 역할.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// 교실 센서.
    Sensor(SensorInfo),
    /// 감독관 뷰.
    Viewer,
}

impl Role {
    /// 증명에 쓰는 역할 문자열.
    pub fn tag(&self) -> String {
        match self {
            Role::Sensor(info) => format!("sensor:{}", info.id),
            Role::Viewer => "viewer".to_owned(),
        }
    }
}

/// 첫 메시지(클라이언트 → 서버).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    /// 프로토콜 버전.
    pub version: u32,
    /// 역할.
    pub role: Role,
    /// 페어링 증명.
    pub proof: String,
}

/// 세션 정보. 센서는 이 시작 시각을 기준으로 NTP로 맞춘 시계로 관측 시각을 찍는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// 세션 시작(유닉스 밀리초).
    pub start_epoch_ms: u64,
}

/// 거부 이유.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    /// 버전이 다르다.
    BadVersion,
    /// 페어링 코드가 틀렸다.
    BadPairing,
    /// 이 출처의 실패가 많아 페어링을 잠시 잠갔다.
    LockedOut,
}

/// 첫 응답(서버 → 클라이언트).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerHello {
    /// 받아들였다.
    Accepted(SessionInfo),
    /// 거부했다.
    Rejected(Rejection),
}

/// 센서 → 서버.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Uplink {
    /// 관측 보고서. 서버는 보고서의 센서 ID를 연결의 센서 ID로 덮어쓴다.
    Report(SensorReport),
    /// 수집기 상태.
    Status(CollectorKind, CollectorStatus),
    /// 살아 있음. 관측이 없는 조용한 교실에서도 연결이 유휴 시간 제한에 걸리지 않게 보낸다.
    Heartbeat,
}

/// 서버 → 감독관 뷰.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Downlink {
    /// 화면 모델.
    View(BuildingView),
}
