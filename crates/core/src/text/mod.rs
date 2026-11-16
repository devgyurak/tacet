//! 감독관에게 보이는 한국어 문구.
//!
//! 알림은 언제나 "확인 필요"이고 판정을 단정하지 않는다(AGENTS.md §3.4). 위치는 추정 위치·신뢰도·
//! 근거를 함께 보인다(설계 §4.7 알림 형태). 앱 창과 웹 대시보드가 같은 문구를 쓰도록 여기에 둔다.

use crate::config::RoomLayout;
use crate::engine::{AlertCandidate, DeviceLabel, EngineWarning, Level, Penalty, Phase};
use crate::ids::SensorPos;
use crate::locate::{Confidence, Localization};
use crate::obs::{CellBand, CollectorKind, CollectorStatus, OffReason, Permission};
use crate::rules::RuleId;

#[cfg(test)]
mod tests;

/// 알려진 회사 ID의 이름(Bluetooth SIG Assigned Numbers, 설계 §11 [S1][S2][S5b]).
pub fn company_name(id: u16) -> Option<&'static str> {
    match id {
        0x004C => Some("Apple"),
        0x0075 => Some("Samsung"),
        0x00E0 => Some("Google"),
        0x01AB => Some("Meta"),
        0x058E => Some("Meta Platforms Technologies"),
        0x0D53 => Some("Luxottica"),
        0x03C2 => Some("Snap"),
        _ => None,
    }
}

/// 의심 강도.
pub fn level(level: Level) -> &'static str {
    match level {
        Level::Low => "낮음",
        Level::Medium => "중간",
        Level::High => "높음",
    }
}

/// 신뢰도.
pub fn confidence(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Low => "낮음",
        Confidence::Medium => "중",
        Confidence::High => "높음",
    }
}

fn position(pos: SensorPos) -> &'static str {
    match pos {
        SensorPos::Front => "앞쪽",
        SensorPos::Back => "뒤쪽",
    }
}

fn band(band: CellBand) -> &'static str {
    match band {
        CellBand::B1 => "B1",
        CellBand::B3 => "B3",
        CellBand::B5 => "B5",
        CellBand::B7 => "B7",
    }
}

/// 규칙 설명.
pub fn rule(rule: RuleId) -> &'static str {
    match rule {
        RuleId::R1 => "R1 폰 제조사 BLE 광고",
        RuleId::R1a => "R1a 스마트 안경 BLE 광고",
        RuleId::R1b => "R1b Fast Pair 액세서리 광고",
        RuleId::R2 => "R2 새 BLE 기기가 교실 경계 안",
        RuleId::R3 => "R3 새 Wi-Fi가 교실 경계 안",
        RuleId::R4 => "R4 랜덤 MAC 핫스팟",
        RuleId::R5 => "R5 숨긴 Wi-Fi",
        RuleId::R6 => "R6 Wi-Fi Direct",
        RuleId::R7 => "R7 LTE 업링크 증가",
    }
}

/// 기기 설명(알림 첫 줄 괄호 안).
pub fn device(label: &DeviceLabel) -> String {
    match label {
        DeviceLabel::Ble { company } => match company.and_then(company_name) {
            Some(name) => format!("BLE 광고 감지 ({name})"),
            None => "BLE 광고 감지".to_owned(),
        },
        DeviceLabel::Wifi {
            bssid,
            hidden: true,
            ..
        } => format!("숨긴 Wi-Fi 감지 (BSSID {bssid})"),
        DeviceLabel::Wifi {
            bssid,
            direct: true,
            ..
        } => format!("Wi-Fi Direct 감지 (BSSID {bssid})"),
        DeviceLabel::Wifi { bssid, .. } => format!("새 Wi-Fi 감지 (BSSID {bssid})"),
        DeviceLabel::Cell { band: b } => format!("LTE 업링크 증가 감지 ({})", band(*b)),
    }
}

/// 추정 위치 줄.
pub fn location(loc: &Localization) -> String {
    let place = match (&loc.room, loc.position, loc.likely_outside) {
        (_, _, true) => "건물 밖 가능성(교실 경계 밖)".to_owned(),
        (Some(room), Some(pos), _) => format!("{room} 교실 {}", position(pos)),
        (Some(room), None, _) => format!("{room} 교실"),
        (None, _, false) => "건물 안, 교실 미정".to_owned(),
    };
    let mut line = format!("추정 위치: {place} · 신뢰도 {}", confidence(loc.confidence));
    if loc.needs_calibration {
        line.push_str(" · 보정 필요");
    }
    line
}

/// 근거 줄: 센서별 RSSI 중앙값, 배치의 나머지 교실은 미탐지(설계 §4.7 알림 형태).
pub fn evidence(loc: &Localization, layout: &RoomLayout) -> String {
    let mut parts: Vec<String> = loc
        .evidence
        .iter()
        .enumerate()
        .map(|(i, (sensor, value))| {
            let name = layout.sensor(sensor).map_or_else(
                || sensor.to_string(),
                |(room, spec)| match spec.position {
                    SensorPos::Front => format!("{} 앞", room.id),
                    SensorPos::Back => format!("{} 뒤", room.id),
                },
            );
            if i == 0 {
                format!("{name} 센서 {value}dBm")
            } else {
                format!("{name} {value}")
            }
        })
        .collect();
    for room in &layout.rooms {
        let seen = room
            .sensors
            .iter()
            .any(|s| loc.evidence.iter().any(|(id, _)| id == &s.id));
        if !seen {
            parts.push(format!("{} 미탐지", room.id));
        }
    }
    format!("근거: {}", parts.join(" / "))
}

/// 알림 전체 문구(여러 줄).
pub fn alert(candidate: &AlertCandidate, layout: &RoomLayout) -> String {
    let mut lines = vec![format!("[확인 필요] {}", device(&candidate.label))];
    lines.push(format!(
        "의심 강도: {} (점수 {})",
        level(candidate.level),
        candidate.score
    ));
    match &candidate.localization {
        Some(loc) => {
            lines.push(location(loc));
            lines.push(evidence(loc, layout));
        }
        None => lines.push("추정 위치: 세기 정보 없음 · 건물 안 탐지".to_owned()),
    }
    if !candidate.penalties.is_empty() {
        let reasons: Vec<&str> = candidate.penalties.iter().map(|p| penalty(*p)).collect();
        lines.push(format!("감점: {}", reasons.join(", ")));
    }
    let rules: Vec<&str> = candidate.rules.iter().map(|r| rule(*r)).collect();
    lines.push(format!("적중 규칙: {}", rules.join(", ")));
    lines.push("자동 판정은 참고용이다. 최종 확인은 감독관이 한다.".to_owned());
    lines.join("\n")
}

/// 감점 사유.
pub fn penalty(penalty: Penalty) -> &'static str {
    match penalty {
        Penalty::StorageZone => "수거폰 보관 위치의 신호",
        Penalty::FarOutside => "모든 센서에서 교실 경계보다 훨씬 약함(건물 밖 가능성)",
    }
}

/// 한 번도 연결되지 않은 센서 줄.
pub const NEVER_CONNECTED: &str = "연결된 적 없음 — 그 교실 자리는 탐지 공백이다";

/// 수집기 이름.
pub fn collector(kind: CollectorKind) -> &'static str {
    match kind {
        CollectorKind::Ble => "BLE 수집",
        CollectorKind::Wifi => "Wi-Fi 수집",
        CollectorKind::Sdr => "LTE 업링크 수집(SDR)",
    }
}

fn permission(permission: Permission) -> &'static str {
    match permission {
        Permission::PreciseLocation => "정밀 위치 권한 없음 — 설정 > 개인 정보 > 위치에서 켠다",
        Permission::Location => {
            "위치 권한 없음 — 시스템 설정 > 개인정보 보호 > 위치 서비스에서 허용한다"
        }
        Permission::Bluetooth => "블루투스 권한 없음 — 시스템 설정에서 블루투스 사용을 허용한다",
    }
}

/// 수집기 상태 한 줄.
pub fn collector_status(kind: CollectorKind, status: &CollectorStatus) -> String {
    let state = match status {
        CollectorStatus::Starting => "시작 중".to_owned(),
        CollectorStatus::Running => "수집 중".to_owned(),
        CollectorStatus::Off(reason) => format!(
            "꺼짐 — {}",
            match reason {
                OffReason::PermissionDenied(p) => permission(*p).to_owned(),
                OffReason::DeviceMissing => "장비 없음".to_owned(),
                OffReason::DriverMissing =>
                    "드라이버 없음 — WinUSB 드라이버를 바인딩한다".to_owned(),
                OffReason::Unsupported(why) => format!("지원하지 않음 ({why})"),
                OffReason::Error(why) => format!("오류 ({why})"),
            }
        ),
    };
    format!("{}: {state}", collector(kind))
}

/// 엔진 경고 한 줄.
pub fn warning(warning: &EngineWarning) -> String {
    match warning {
        EngineWarning::CalibrationMissing(sensor) => {
            format!(
                "{sensor} 센서: 교실 보정값 없음 — 교실 경계 판정(R2·R3, 교실 안팎)을 하지 않는다"
            )
        }
        EngineWarning::UnknownSensor(sensor) => {
            format!("{sensor} 센서: 배치에 없는 센서라 보고서를 버렸다")
        }
        EngineWarning::MacWhitelistNotApplicable(sensor) => format!(
            "{sensor} 센서(macOS): BLE 주소가 없어 MAC 화이트리스트가 먹지 않는다 — 의료기기는 제조사 데이터로 등록한다"
        ),
        EngineWarning::WifiCalibrationMissing(sensor) => format!(
            "{sensor} 센서: 와이파이 보정값 없음 — 와이파이의 교실 경계 판정(R3, 교실 안팎)을 하지 않는다"
        ),
    }
}

/// 세션 단계 한 줄.
pub fn phase(phase: Phase, remaining_ms: u64) -> String {
    match phase {
        Phase::Baseline => format!("기준선 수집 중 — {}초 남음", remaining_ms.div_ceil(1000)),
        Phase::Watching => "감시 중".to_owned(),
    }
}
