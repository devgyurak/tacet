//! 시험장 무선 기기 탐지기의 순수 판정 로직.
//!
//! 이 크레이트는 IO·시계·async·난수·OS 분기를 쓰지 않는다(AGENTS.md §2 규칙 1). 입력은 관측값과
//! 설정이고 출력은 규칙 적중, 의심 점수, 위치 판정, 알림 후보뿐이다. 단독 모드와 서버 모드가 같은
//! [`engine::Engine`]으로 판정한다.

pub mod baseline;
pub mod calibrate;
pub mod config;
pub mod device;
pub mod engine;
pub mod explain;
pub mod ids;
pub mod locate;
pub mod obs;
pub mod order;
pub mod report;
pub mod rules;
#[cfg(feature = "testkit")]
pub mod testkit;
pub mod text;
pub mod time;
pub mod view;

pub use engine::{AlertCandidate, Engine, EngineWarning, Level, Phase};
pub use ids::{BleId, Mac, Platform, RoomId, SensorId, SensorPos, ServiceUuid};
pub use obs::{
    Band, BleObs, CellBand, CollectorEvent, CollectorKind, CollectorStatus, Observation, OffReason,
    Permission, RfObs, SensorInfo, SensorReport, Ssid, WifiObs,
};
pub use time::Timestamp;
