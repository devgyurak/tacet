//! 앱 설정(TOML). 판정 설정·배치·화이트리스트는 core 타입을 그대로 쓴다.

use std::net::SocketAddr;
use std::path::PathBuf;

use serde::Deserialize;
use tacet_core::config::{DetectorConfig, LayoutError, RoomLayout, SettingError, Whitelist};
use tacet_core::ids::{Platform, RoomId, SensorId, SensorPos};
use tacet_core::obs::SensorInfo;

#[cfg(test)]
mod tests;

/// 실행 모드(설계 §5.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// 수집 + 판정 + 알림을 한 대에서.
    Standalone,
    /// 수집만 하고 서버로 보낸다.
    Sensor,
    /// 건물 단위 판정과 웹 대시보드.
    Server,
    /// 서버 화면을 앱 창으로.
    Viewer,
}

impl Mode {
    /// 화면에 보일 이름. 감독관이 쓰는 기본 모드는 이 노트북 혼자 감지하는 단독 모드다. 서버를 쓰는 건물 단위 구성
    /// (센서·서버·본부 화면 보기)은 설치·시험을 맡은 개발자만 쓴다.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Standalone => "감독관 화면",
            Mode::Sensor => "교실 센서 (개발자용)",
            Mode::Server => "시험본부 서버 (개발자용)",
            Mode::Viewer => "본부 화면 보기 (개발자용)",
        }
    }
}

/// 이 컴퓨터가 센서로서 놓인 자리.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SensorSection {
    /// 센서 ID(배치의 센서 ID와 같아야 한다).
    pub id: String,
    /// 교실.
    pub room: String,
    /// 교실 안 위치.
    pub position: SensorPos,
}

/// 센서·감독관 뷰가 붙을 서버.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RemoteSection {
    /// 서버 주소. 비우면 mDNS로 찾는다.
    pub address: Option<SocketAddr>,
    /// 서버 화면의 페어링 코드(센서는 센서 코드, 감독관 뷰는 감독관 뷰 코드).
    pub code: String,
    /// 서버 화면의 지문. 적으면 첫 연결에서도 이 지문인 서버에만 붙는다(첫 연결 가로채기 방어). 비우면 첫
    /// 연결의 지문을 믿고 고정하므로, 센서 화면의 지문을 서버 화면과 꼭 대조한다.
    pub fingerprint: Option<String>,
}

/// 서버 모드 설정.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerSection {
    /// 센서·감독관 뷰 연결을 받을 주소(내부망 인터페이스).
    pub bind: SocketAddr,
    /// 웹 대시보드 주소(내부망 인터페이스).
    pub dashboard_bind: SocketAddr,
    /// 건물 이름(mDNS 인스턴스).
    #[serde(default = "default_building")]
    pub building: String,
}

fn default_building() -> String {
    "시험본부".to_owned()
}

/// SDR 설정.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct SdrSection {
    /// SoapySDR 장치 인자. 비우면 SDR을 쓰지 않는다.
    pub device_args: Option<String>,
}

/// 앱 설정.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppConfig {
    /// 실행 모드.
    pub mode: Mode,
    /// 단독·센서 모드에서 이 컴퓨터의 센서 자리.
    pub sensor: Option<SensorSection>,
    /// 센서·감독관 뷰 모드에서 붙을 서버.
    pub remote: Option<RemoteSection>,
    /// 서버 모드 설정.
    pub server: Option<ServerSection>,
    /// 판정 설정.
    #[serde(default)]
    pub detector: DetectorConfig,
    /// 교실 배치와 보정값.
    #[serde(default)]
    pub layout: RoomLayout,
    /// 화이트리스트.
    #[serde(default)]
    pub whitelist: Whitelist,
    /// SDR.
    #[serde(default)]
    pub sdr: SdrSection,
    /// 앱 상태 폴더(서버 인증서, 고정 지문, 보고서). 비우면 OS 기본 위치. 저장소 밖이어야 한다.
    pub state_dir: Option<PathBuf>,
}

/// 설정 오류.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// TOML 형식 오류.
    #[error("설정 형식이 틀렸다: {0}")]
    Toml(#[from] toml::de::Error),
    /// 배치 오류.
    #[error(transparent)]
    Layout(#[from] LayoutError),
    /// 모드에 필요한 절이 없다.
    #[error("{mode} 모드에는 [{section}] 절이 필요하다")]
    Missing {
        /// 모드 이름.
        mode: &'static str,
        /// 빠진 절.
        section: &'static str,
    },
    /// 센서가 배치에 없다.
    #[error("센서 {0}가 [layout]에 없다")]
    SensorNotInLayout(String),
    /// 판정 설정·화이트리스트 값 오류.
    #[error(transparent)]
    Setting(#[from] SettingError),
    /// 서버 지문 형식이 틀렸다.
    #[error("[remote] fingerprint 형식이 틀렸다 — 서버 화면의 지문을 그대로 적는다")]
    BadFingerprint,
}

impl AppConfig {
    /// TOML을 읽고 모드에 맞는지 확인한다.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let config: AppConfig = toml::from_str(text)?;
        config.layout.validate()?;
        config.detector.validate()?;
        config.whitelist.validate()?;
        if config
            .expected_fingerprint()
            .is_some_and(|parsed| parsed.is_none())
        {
            return Err(ConfigError::BadFingerprint);
        }
        let missing = |section| ConfigError::Missing {
            mode: config.mode.label(),
            section,
        };
        match config.mode {
            Mode::Standalone => {
                let sensor = config.sensor.as_ref().ok_or_else(|| missing("sensor"))?;
                if config.layout.sensor(&SensorId::new(&sensor.id)).is_none() {
                    return Err(ConfigError::SensorNotInLayout(sensor.id.clone()));
                }
            }
            Mode::Sensor => {
                config.sensor.as_ref().ok_or_else(|| missing("sensor"))?;
                config.remote.as_ref().ok_or_else(|| missing("remote"))?;
            }
            Mode::Server => {
                config.server.as_ref().ok_or_else(|| missing("server"))?;
            }
            Mode::Viewer => {
                config.remote.as_ref().ok_or_else(|| missing("remote"))?;
            }
        }
        Ok(config)
    }

    /// `[remote] fingerprint`. 적지 않았으면 `None`, 적었는데 형식이 틀렸으면 `Some(None)`.
    pub fn expected_fingerprint(&self) -> Option<Option<tacet_net::Fingerprint>> {
        let text = self.remote.as_ref()?.fingerprint.as_deref()?;
        Some(tacet_net::Fingerprint::parse(text))
    }

    /// 서버 모드의 듣는 주소 경고. 모든 인터페이스(0.0.0.0)나 사설망이 아닌 주소면 감독관에게 알린다.
    pub fn bind_warnings(&self) -> Vec<String> {
        let Some(server) = &self.server else {
            return Vec::new();
        };
        [("센서 연결", server.bind), ("웹 대시보드", server.dashboard_bind)]
            .into_iter()
            .filter(|(_, addr)| !is_internal(addr.ip()))
            .map(|(what, addr)| {
                format!("주의: {what} 주소 {addr}는 시험장 내부망 인터페이스가 아니다 — 내부망 주소로 바꾼다")
            })
            .collect()
    }

    /// 앱 상태 폴더.
    pub fn state_dir(&self) -> PathBuf {
        self.state_dir
            .clone()
            .unwrap_or_else(crate::platform::state_dir)
    }

    /// 이 컴퓨터의 센서 정보(단독·센서 모드).
    pub fn sensor_info(&self, platform: Platform) -> Option<SensorInfo> {
        self.sensor.as_ref().map(|s| SensorInfo {
            id: SensorId::new(&s.id),
            room: RoomId::new(&s.room),
            position: s.position,
            platform,
        })
    }
}

/// 내부망 주소인지: 루프백, 사설망(RFC 1918), 링크 로컬, IPv6 ULA. 모든 인터페이스(미지정 주소)는 아니다.
fn is_internal(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
        }
    }
}

/// 명령줄에서 `--config <경로>`를 찾는다. 없으면 `default`.
pub fn config_path_from_args(args: impl IntoIterator<Item = String>, default: PathBuf) -> PathBuf {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--config"
            && let Some(path) = args.next()
        {
            return PathBuf::from(path);
        }
    }
    default
}
