//! 감독관이 바로 이해하는 쉬운 말 설명.
//!
//! [`crate::text`]의 문구는 규칙 ID·dBm·BSSID 같은 기술 용어를 쓴다(개발자 화면, 로그, 보고서). 여기는 같은 판정을
//! "무엇이 보였나 · 어디인가 · 얼마나 확실한가 · 무엇을 하면 되나"로 풀어 쓴다. 판단은 하지 않고 엔진이 정한 값을 말로
//! 옮길 뿐이다(AGENTS.md §2 규칙 11). 알림은 언제나 "확인 필요"의 어조이고 사람을 지목하지 않는다(AGENTS.md §3.4).

use serde::{Deserialize, Serialize};

use crate::config::RoomLayout;
use crate::engine::{AlertCandidate, DeviceLabel, EngineWarning, Level, Penalty};
use crate::ids::{RoomId, SensorId, SensorPos};
use crate::locate::{Confidence, Localization};
use crate::obs::{CollectorKind, CollectorStatus, OffReason, Permission};
use crate::rules::{GLASSES_VENDOR_IDS, PHONE_VENDOR_IDS, RuleId};
use crate::text;

#[cfg(test)]
mod tests;

/// 안내의 무게. 화면은 이 값으로만 색과 표시 여부를 고른다 — 무엇이 문제인지는 여기서 정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// 정상.
    Ok,
    /// 참고(추가 장비 없음, 설정 안내). 감독관 기본 화면에는 보이지 않고 개발자 화면에만 보인다.
    Info,
    /// 주의: 탐지는 되지만 정확도가 떨어진다.
    Warning,
    /// 문제: 탐지 공백이 생겼다.
    Problem,
}

/// 화면이 안내 옆에 붙일 수 있는 바로 가기. 어떤 안내에 붙일지는 여기서 정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fix {
    /// 운영체제의 위치 권한 설정 화면 열기.
    OpenLocationSettings,
    /// 앱 안 교실 보정(`docs/rules/L-room-calibration.md`).
    Calibrate,
}

/// 문제 안내 한 건: 무엇이 문제인지, 어디서인지, 그래서 무엇을 놓칠 수 있는지, 무엇을 하면 되는지.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Notice {
    /// 무게.
    pub severity: Severity,
    /// 어디서(예: "3-2 교실 앞쪽 노트북").
    pub place: String,
    /// 무엇이 문제인지.
    pub problem: String,
    /// 그래서 생기는 일.
    pub effect: String,
    /// 할 일.
    pub action: String,
    /// 개발자용 기술 설명.
    pub technical: String,
    /// 바로 가기(있으면).
    pub fix: Option<Fix>,
}

/// 센서 자리 이름: 배치에 있으면 "3-2 교실 앞쪽 노트북"(자리 설명이 있으면 그것), 없으면 센서 ID.
pub fn sensor_place(layout: &RoomLayout, sensor: &SensorId) -> String {
    match layout.sensor(sensor) {
        Some((room, spec)) => match &spec.place {
            Some(place) => format!("{} 교실 {place} 노트북", room.id),
            None => format!("{} 교실 {} 노트북", room.id, side(spec.position)),
        },
        None => format!("{sensor} 노트북(교실 배치에 없음)"),
    }
}

fn side(pos: SensorPos) -> &'static str {
    match pos {
        SensorPos::Front => "앞쪽",
        SensorPos::Back => "뒤쪽",
    }
}

/// 의심 정도 이름.
pub fn level(level: Level) -> &'static str {
    match level {
        Level::High => "의심 높음",
        Level::Medium => "의심 보통",
        Level::Low => "의심 낮음",
    }
}

fn is_glasses(company: Option<u16>) -> bool {
    company.is_some_and(|c| GLASSES_VENDOR_IDS.contains(&c))
}

fn is_phone(company: Option<u16>) -> bool {
    company.is_some_and(|c| PHONE_VENDOR_IDS.contains(&c))
}

/// 무엇이 보였는지(제목). 식별자는 넣지 않는다 — 기술 정보에만 있다.
pub fn title(label: &DeviceLabel) -> String {
    match label {
        DeviceLabel::Ble { company } if is_glasses(*company) => {
            match company.and_then(text::company_name) {
                Some(name) => format!("스마트 안경으로 보이는 블루투스 신호 ({name})"),
                None => "스마트 안경으로 보이는 블루투스 신호".to_owned(),
            }
        }
        DeviceLabel::Ble { company } if is_phone(*company) => {
            match company.and_then(text::company_name) {
                Some(name) => format!("휴대폰일 수 있는 블루투스 신호 ({name})"),
                None => "휴대폰일 수 있는 블루투스 신호".to_owned(),
            }
        }
        DeviceLabel::Ble { .. } => "새 블루투스 기기 신호".to_owned(),
        DeviceLabel::Wifi { hidden: true, .. } => {
            "이름을 숨긴 와이파이 (핫스팟일 수 있음)".to_owned()
        }
        DeviceLabel::Wifi { direct: true, .. } => {
            "기기끼리 직접 잇는 와이파이 (Wi-Fi Direct)".to_owned()
        }
        DeviceLabel::Wifi { .. } => "새 와이파이 (핫스팟일 수 있음)".to_owned(),
        DeviceLabel::Cell { .. } => "휴대폰 데이터 전송으로 보이는 전파 증가".to_owned(),
    }
}

/// 규칙이 뜻하는 것(근거 한 줄).
pub fn reason(rule: RuleId) -> &'static str {
    match rule {
        RuleId::R1 => {
            "휴대폰 제조사(Apple·Samsung·Google)의 블루투스 신호입니다. 휴대폰일 수 있지만 같은 제조사의 이어폰·시계·보청기일 수도 있습니다."
        }
        RuleId::R1a => {
            "스마트 안경 제조사(Meta·Luxottica·Snap)의 블루투스 신호입니다. 스마트 안경일 수 있습니다."
        }
        RuleId::R1b => "휴대폰과 짝지어 쓰는 액세서리(무선 이어폰 등)의 신호입니다.",
        RuleId::R2 => "시험 전에는 없던 블루투스 기기가 교실 안 거리에서 잡혔습니다.",
        RuleId::R3 => "시험 전에는 없던 와이파이가 교실 안 거리에서 잡혔습니다.",
        RuleId::R4 => "휴대폰 핫스팟이 흔히 쓰는 임의 주소의 와이파이입니다.",
        RuleId::R5 => "이름을 숨긴 와이파이입니다. 숨긴 핫스팟일 수 있습니다.",
        RuleId::R6 => "기기끼리 직접 잇는 와이파이(Wi-Fi Direct)입니다.",
        RuleId::R7 => "휴대폰이 데이터를 보낼 때 쓰는 대역의 전파가 시험 전보다 크게 늘었습니다.",
    }
}

fn caveat(penalty: Penalty) -> &'static str {
    match penalty {
        Penalty::StorageZone => {
            "수거한 휴대폰 보관 위치에서 나온 신호일 수 있어 의심 정도를 낮췄습니다."
        }
        Penalty::FarOutside => {
            "모든 교실에서 신호가 아주 약해 건물 밖 신호일 수 있어 의심 정도를 낮췄습니다."
        }
    }
}

/// 어디인지.
pub fn place(localization: Option<&Localization>) -> String {
    let Some(loc) = localization else {
        return "건물 안 (신호 세기 정보가 없어 교실을 알 수 없음)".to_owned();
    };
    match (&loc.room, loc.position, loc.likely_outside) {
        (_, _, true) => "건물 밖일 수 있음 (모든 교실에서 신호가 아주 약함)".to_owned(),
        (Some(room), Some(pos), _) => format!("{room} 교실 {}", side(pos)),
        (Some(room), None, _) => format!("{room} 교실"),
        (None, _, false) => "건물 안 (어느 교실인지 정하지 못함)".to_owned(),
    }
}

/// 위치가 얼마나 확실한지.
pub fn certainty(localization: Option<&Localization>) -> String {
    let Some(loc) = localization else {
        return "위치를 알 수 없어 모든 교실에 알렸습니다.".to_owned();
    };
    if loc.room.is_none() {
        return "교실을 정하지 못해 모든 교실에 알렸습니다.".to_owned();
    }
    match loc.confidence {
        Confidence::High => {
            "위치 확실도 높음 — 이 교실 노트북의 신호가 다른 교실보다 확실히 셉니다.".to_owned()
        }
        Confidence::Medium => {
            "위치 확실도 보통 — 이 교실 신호가 가장 세지만 옆 교실과 차이가 크지 않습니다."
                .to_owned()
        }
        Confidence::Low => {
            "위치 확실도 낮음 — 옆 교실일 수도 있어 옆 교실에도 함께 알렸습니다.".to_owned()
        }
    }
}

/// 할 일. 사람을 지목하지 않고 자리를 둘러보게 한다.
pub fn guide(localization: Option<&Localization>) -> String {
    match localization {
        Some(Localization { likely_outside: true, .. }) => {
            "건물 밖 신호일 수 있습니다. 교실 창가·복도 쪽을 함께 살펴봐 주세요.".to_owned()
        }
        Some(Localization { room: Some(room), position: Some(pos), .. }) => {
            format!("{room} 교실 {}을 중심으로 둘러보고, 켜져 있는 전자기기가 있는지 확인해 주세요.", side(*pos))
        }
        Some(Localization { room: Some(room), .. }) => {
            format!("{room} 교실을 둘러보고, 켜져 있는 전자기기가 있는지 확인해 주세요.")
        }
        _ => "교실을 정하지 못했습니다. 각 교실 감독관이 교실을 둘러보고 켜져 있는 전자기기가 있는지 확인해 주세요.".to_owned(),
    }
}

/// 알림 하나의 쉬운 말 설명.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertExplanation {
    /// 무엇이 보였나.
    pub title: String,
    /// 의심 정도 이름.
    pub level: String,
    /// 어디인가.
    pub place: String,
    /// 얼마나 확실한가.
    pub certainty: String,
    /// 왜 알렸나(규칙마다 한 줄).
    pub reasons: Vec<String>,
    /// 감안할 점(감점, 보정 없음).
    pub caveats: Vec<String>,
    /// 무엇을 하면 되나.
    pub guide: String,
    /// 개발자용 기술 정보(점수, 규칙 ID, 센서별 세기, 식별자).
    pub technical: Vec<String>,
}

/// 알림 후보를 쉬운 말로 풀어 쓴다.
pub fn alert(candidate: &AlertCandidate, layout: &RoomLayout) -> AlertExplanation {
    let loc = candidate.localization.as_ref();
    let mut caveats: Vec<String> = candidate
        .penalties
        .iter()
        .map(|p| caveat(*p).to_owned())
        .collect();
    if loc.is_some_and(|l| l.needs_calibration) {
        caveats.push(
            "이 교실은 신호 기준값(교실 보정)이 없어 위치가 부정확할 수 있습니다.".to_owned(),
        );
    }
    let rules: Vec<&str> = candidate.rules.iter().map(|r| text::rule(*r)).collect();
    let mut technical = vec![
        text::device(&candidate.label),
        format!("점수 {} · 적중 규칙: {}", candidate.score, rules.join(", ")),
    ];
    if let Some(loc) = loc {
        technical.push(text::location(loc));
        technical.push(text::evidence(loc, layout));
    }
    AlertExplanation {
        title: title(&candidate.label),
        level: level(candidate.level).to_owned(),
        place: place(loc),
        certainty: certainty(loc),
        reasons: candidate
            .rules
            .iter()
            .map(|r| reason(*r).to_owned())
            .collect(),
        caveats,
        guide: guide(loc),
        technical,
    }
}

/// 수집기의 쉬운 이름.
pub fn collector(kind: CollectorKind) -> &'static str {
    match kind {
        CollectorKind::Ble => "블루투스 신호 감지",
        CollectorKind::Wifi => "와이파이 신호 감지",
        CollectorKind::Sdr => "휴대폰 전파 감지 장비(SDR)",
    }
}

/// 추가 장비인지. 없어도 기본 탐지(블루투스·와이파이)는 된다.
fn optional(kind: CollectorKind) -> bool {
    matches!(kind, CollectorKind::Sdr)
}

fn missed(kind: CollectorKind) -> &'static str {
    match kind {
        CollectorKind::Ble => "이 자리에서 휴대폰·스마트 안경의 블루투스 신호를 놓칠 수 있습니다.",
        CollectorKind::Wifi => "이 자리에서 휴대폰 핫스팟(숨긴 와이파이 포함)을 놓칠 수 있습니다.",
        CollectorKind::Sdr => {
            "휴대폰 데이터 전송 증가를 볼 수 없습니다. 블루투스·와이파이 감지는 계속됩니다."
        }
    }
}

fn permission_problem(permission: Permission) -> (&'static str, &'static str) {
    match permission {
        Permission::Bluetooth => (
            "블루투스 사용 권한이 없습니다.",
            "운영체제 설정의 개인정보 보호(블루투스)에서 이 앱을 허용한 뒤 앱을 다시 시작하세요.",
        ),
        Permission::PreciseLocation => (
            "Windows 정밀 위치 권한이 꺼져 있어 와이파이 주소를 볼 수 없습니다.",
            "설정 > 개인 정보 및 보안 > 위치에서 위치 서비스와 '정밀 위치'를 켜세요.",
        ),
        Permission::Location => (
            "macOS 위치 권한이 없어 와이파이 주소를 볼 수 없습니다.",
            "시스템 설정 > 개인정보 보호 및 보안 > 위치 서비스에서 이 앱을 허용하세요.",
        ),
    }
}

/// 수집기 상태의 쉬운 말과 무게.
pub fn collector_state(kind: CollectorKind, status: &CollectorStatus) -> (Severity, String) {
    match status {
        CollectorStatus::Starting => (Severity::Ok, "시작하는 중".to_owned()),
        CollectorStatus::Running => (Severity::Ok, "정상 작동 중".to_owned()),
        CollectorStatus::Off(reason) => {
            let severity = match (optional(kind), reason) {
                // 추가 장비가 꽂혀 있지 않거나 아직 없는 기능이면 참고만 — 기본 탐지는 그대로다.
                (true, OffReason::DeviceMissing | OffReason::Unsupported(_)) => Severity::Info,
                (true, _) => Severity::Warning,
                (false, _) => Severity::Problem,
            };
            (severity, off_problem(kind, reason).0)
        }
    }
}

fn off_problem(kind: CollectorKind, reason: &OffReason) -> (String, String) {
    let device = match kind {
        CollectorKind::Ble => "블루투스",
        CollectorKind::Wifi => "와이파이",
        _ => "장비",
    };
    match reason {
        OffReason::PermissionDenied(p) => {
            let (problem, action) = permission_problem(*p);
            (problem.to_owned(), action.to_owned())
        }
        OffReason::DeviceMissing if optional(kind) => (
            "장비가 연결돼 있지 않습니다.".to_owned(),
            "쓰려면 장비를 USB에 꽂고 앱을 다시 시작하세요.".to_owned(),
        ),
        OffReason::DeviceMissing => (
            format!("이 노트북에서 {device} 장치를 찾지 못했습니다."),
            format!("{device}가 켜져 있는지(비행기 모드가 아닌지) 확인하세요."),
        ),
        OffReason::DriverMissing => (
            "장비는 보이지만 드라이버가 설치되지 않았습니다.".to_owned(),
            "설치 안내에 따라 WinUSB 드라이버를 설치하세요.".to_owned(),
        ),
        OffReason::Unsupported(_) => (
            "이 버전의 앱에는 이 기능이 들어 있지 않습니다.".to_owned(),
            "조치할 것 없음.".to_owned(),
        ),
        OffReason::Error(_) => (
            "오류로 멈췄습니다.".to_owned(),
            "앱을 다시 시작해 보고, 계속되면 개발자에게 알려 주세요.".to_owned(),
        ),
    }
}

/// 꺼진 수집기의 안내. 켜져 있으면 `None`.
pub fn collector_notice(
    layout: &RoomLayout,
    sensor: &SensorId,
    kind: CollectorKind,
    status: &CollectorStatus,
) -> Option<Notice> {
    let CollectorStatus::Off(reason) = status else {
        return None;
    };
    let (severity, _) = collector_state(kind, status);
    let (problem, action) = off_problem(kind, reason);
    Some(Notice {
        severity,
        place: sensor_place(layout, sensor),
        problem: format!("{}: {problem}", collector(kind)),
        effect: missed(kind).to_owned(),
        action,
        technical: format!("{sensor} — {}", text::collector_status(kind, status)),
        fix: matches!(
            reason,
            OffReason::PermissionDenied(Permission::Location | Permission::PreciseLocation)
        )
        .then_some(Fix::OpenLocationSettings),
    })
}

/// 연결이 끊겼거나 한 번도 연결되지 않은 센서의 안내.
pub fn sensor_offline(layout: &RoomLayout, sensor: &SensorId, ever_connected: bool) -> Notice {
    Notice {
        severity: Severity::Problem,
        place: sensor_place(layout, sensor),
        problem: if ever_connected {
            "노트북 연결이 끊겼습니다.".to_owned()
        } else {
            "노트북이 아직 연결되지 않았습니다.".to_owned()
        },
        effect: "이 자리의 신호를 받지 못해 탐지 공백이 생깁니다.".to_owned(),
        action:
            "그 노트북이 켜져 있고 앱이 실행 중인지, 시험장 네트워크에 연결돼 있는지 확인하세요."
                .to_owned(),
        technical: if ever_connected {
            format!("{sensor} 센서 연결 끊김")
        } else {
            format!("{sensor} — {}", text::NEVER_CONNECTED)
        },
        fix: None,
    }
}

/// 엔진 경고의 안내.
pub fn warning(layout: &RoomLayout, warning: &EngineWarning) -> Notice {
    let technical = text::warning(warning);
    match warning {
        EngineWarning::CalibrationMissing(sensor) => Notice {
            severity: Severity::Warning,
            place: sensor_place(layout, sensor),
            problem: "이 자리의 신호 기준값(교실 보정)이 설정되지 않았습니다.".to_owned(),
            effect: "기기가 교실 안인지 밖인지 가리지 못해 일부 알림이 나오지 않고 위치가 부정확할 수 있습니다.".to_owned(),
            action: "아래 '교실 보정하기'를 눌러 휴대폰으로 보정하세요(1분쯤 걸립니다). 건물 단위 구성은 설치 담당자가 설정 파일에 넣습니다.".to_owned(),
            technical,
            fix: Some(Fix::Calibrate),
        },
        EngineWarning::UnknownSensor(sensor) => Notice {
            severity: Severity::Warning,
            place: sensor_place(layout, sensor),
            problem: "교실 배치에 없는 노트북이 신호를 보내고 있습니다.".to_owned(),
            effect: "이 노트북이 보낸 신호는 판정에 쓰지 않습니다.".to_owned(),
            action: "설정의 교실 배치에 이 노트북을 넣거나, 노트북의 센서 이름을 확인하세요.".to_owned(),
            technical,
            fix: None,
        },
        EngineWarning::MacWhitelistNotApplicable(sensor) => Notice {
            severity: Severity::Info,
            place: sensor_place(layout, sensor),
            problem: "이 Mac은 블루투스 기기 주소를 볼 수 없어, 주소로 등록한 예외 기기(의료기기·보청기 등)를 블루투스에서는 거르지 못합니다.".to_owned(),
            effect: "주소로만 등록한 예외 기기가 이 자리에서 알림으로 뜰 수 있습니다. 와이파이 예외에는 영향이 없습니다.".to_owned(),
            action: "블루투스 예외 기기는 제조사 정보로 등록하세요.".to_owned(),
            technical,
            fix: None,
        },
        EngineWarning::WifiCalibrationMissing(sensor) => Notice {
            severity: Severity::Warning,
            place: sensor_place(layout, sensor),
            problem: "이 자리의 와이파이 신호 기준값(교실 보정)이 설정되지 않았습니다.".to_owned(),
            effect: "핫스팟이 교실 안인지 밖인지 가리지 못해 일부 알림이 나오지 않고 위치가 부정확할 수 있습니다.".to_owned(),
            action: "아래 '교실 보정하기'로 휴대폰 블루투스와 핫스팟을 함께 재세요.".to_owned(),
            technical,
            fix: Some(Fix::Calibrate),
        },
    }
}

/// 알릴 교실 이름 목록(쉬운 말). 비어 있으면 모든 교실.
pub fn audience(rooms: &[RoomId]) -> String {
    if rooms.is_empty() {
        return "모든 교실".to_owned();
    }
    let names: Vec<String> = rooms.iter().map(|r| format!("{r} 교실")).collect();
    names.join(", ")
}
