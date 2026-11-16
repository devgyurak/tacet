//! 감독관 화면의 모델. 서버가 만들어 웹 대시보드와 데스크톱 앱에 똑같이 보낸다.
//!
//! 화면은 판단하지 않는다(AGENTS.md §2 규칙 11). 문구는 모두 [`crate::explain`](쉬운 말)과 [`crate::text`](기술
//! 용어)가 만들고, 무엇을 감독관에게 보이고 무엇을 개발자 화면에만 둘지도 여기서 [`Severity`]로 정한다.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::config::RoomLayout;
use crate::engine::{AlertCandidate, Engine, Level, Phase};
use crate::explain::{self, Notice, Severity};
use crate::ids::{RoomId, SensorId};
use crate::obs::{CollectorKind, CollectorStatus};
use crate::text;

#[cfg(test)]
mod tests;

/// 알림 한 건의 화면 모델.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertView {
    /// 의심 강도.
    pub level: Level,
    /// 알릴 교실.
    pub rooms: Vec<RoomId>,
    /// 교실 미정이라 모든 교실에 알린다. 어느 교실 화면에 보일지는 이 값으로만 정한다(화면은 판단하지 않는다).
    pub everyone: bool,
    /// 알린 교실(쉬운 말).
    pub audience: String,
    /// 쉬운 말 설명.
    pub explain: explain::AlertExplanation,
    /// 기술 용어로 쓴 전체 문구(로그·개발자 화면).
    pub text: String,
}

impl AlertView {
    /// 알림 후보에서 만든다.
    pub fn from_candidate(candidate: &AlertCandidate, layout: &RoomLayout) -> Self {
        Self {
            level: candidate.level,
            rooms: candidate.notify_rooms.clone(),
            everyone: candidate.notify_rooms.is_empty(),
            audience: explain::audience(&candidate.notify_rooms),
            explain: explain::alert(candidate, layout),
            text: text::alert(candidate, layout),
        }
    }
}

/// 수집기 한 개의 화면 모델.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectorView {
    /// 쉬운 이름.
    pub name: String,
    /// 쉬운 상태.
    pub state: String,
    /// 무게.
    pub severity: Severity,
    /// 기술 용어로 쓴 상태.
    pub technical: String,
}

/// 센서 한 대(교실 노트북)의 화면 모델.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensorView {
    /// 센서.
    pub sensor: SensorId,
    /// 자리(예: "3-2 교실 앞쪽 노트북").
    pub place: String,
    /// 연결돼 있는지.
    pub connected: bool,
    /// 전체 무게: 수집기 중 가장 무거운 것, 연결이 없으면 문제.
    pub severity: Severity,
    /// 한 줄 요약.
    pub summary: String,
    /// 수집기 상태(정렬됨).
    pub collectors: Vec<CollectorView>,
}

/// 단계 화면 모델.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PhaseView {
    /// 감시 중인지(기준선이 끝났는지).
    pub watching: bool,
    /// 제목.
    pub title: String,
    /// 설명.
    pub detail: String,
    /// 기준선이 끝나기까지 남은 초.
    pub remaining_secs: u64,
}

/// 건물 전체 화면 모델.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BuildingView {
    /// 단계.
    pub phase: PhaseView,
    /// 배치의 교실(화면의 "내 교실" 고르기용).
    pub rooms: Vec<RoomId>,
    /// 현재 알림(강도 높은 순).
    pub alerts: Vec<AlertView>,
    /// 센서 상태.
    pub sensors: Vec<SensorView>,
    /// 문제 안내(무거운 순). 참고(`Info`)는 개발자 화면에만 보인다.
    pub notices: Vec<Notice>,
}

/// 센서별 수집기 상태 표.
pub type CollectorTable = BTreeMap<SensorId, (bool, BTreeMap<CollectorKind, CollectorStatus>)>;

/// 단계 화면 모델을 만든다.
pub fn phase_view(phase: Phase, remaining_ms: u64) -> PhaseView {
    match phase {
        Phase::Baseline => PhaseView {
            watching: false,
            title: "준비 중 — 시험 전 주변 신호를 익히는 중".to_owned(),
            detail: "시험 전부터 있던 기기(감독관 장비, 교실 설비 등)는 알림에서 뺍니다. 이 동안에는 알림이 나오지 않습니다."
                .to_owned(),
            remaining_secs: remaining_ms.div_ceil(1000),
        },
        Phase::Watching => PhaseView {
            watching: true,
            title: "감시 중".to_owned(),
            detail: "시험 시작 뒤 새로 나타난 무선 신호를 지켜보고 있습니다.".to_owned(),
            remaining_secs: 0,
        },
    }
}

fn sensor_view(
    layout: &RoomLayout,
    sensor: SensorId,
    entry: Option<&(bool, BTreeMap<CollectorKind, CollectorStatus>)>,
) -> SensorView {
    let place = explain::sensor_place(layout, &sensor);
    let Some((connected, kinds)) = entry else {
        return SensorView {
            sensor,
            place,
            connected: false,
            severity: Severity::Problem,
            summary: "연결된 적 없음".to_owned(),
            collectors: Vec::new(),
        };
    };
    let collectors: Vec<CollectorView> = kinds
        .iter()
        .map(|(kind, status)| {
            let (severity, state) = explain::collector_state(*kind, status);
            CollectorView {
                name: explain::collector(*kind).to_owned(),
                state,
                severity,
                technical: text::collector_status(*kind, status),
            }
        })
        .collect();
    let worst = collectors
        .iter()
        .map(|c| c.severity)
        .max()
        .unwrap_or(Severity::Ok);
    let (severity, summary) = match (*connected, worst) {
        (false, _) => (Severity::Problem, "연결 끊김".to_owned()),
        (true, Severity::Problem) => (
            Severity::Problem,
            "문제 있음 — 일부 신호를 받지 못함".to_owned(),
        ),
        (true, Severity::Warning) => (Severity::Warning, "주의 — 추가 장비에 문제".to_owned()),
        (true, _) => (worst, "정상".to_owned()),
    };
    SensorView {
        sensor,
        place,
        connected: *connected,
        severity,
        summary,
        collectors,
    }
}

/// 엔진과 센서 상태로 화면 모델을 만든다.
pub fn building_view(engine: &Engine, collectors: &CollectorTable) -> BuildingView {
    let layout = engine.layout();
    let mut alerts: Vec<AlertView> = engine
        .snapshot()
        .iter()
        .map(|c| AlertView::from_candidate(c, layout))
        .collect();
    alerts.sort_by(|a, b| b.level.cmp(&a.level).then_with(|| a.text.cmp(&b.text)));
    // 배치의 센서는 연결된 적이 없어도 보인다 — 조용히 빠진 센서는 그 교실의 탐지 공백이다.
    let mut ids: BTreeSet<SensorId> = layout
        .rooms
        .iter()
        .flat_map(|r| r.sensors.iter().map(|s| s.id.clone()))
        .collect();
    ids.extend(collectors.keys().cloned());
    let sensors: Vec<SensorView> = ids
        .into_iter()
        .map(|sensor| sensor_view(layout, sensor.clone(), collectors.get(&sensor)))
        .collect();

    let mut notices: Vec<Notice> = Vec::new();
    for sensor in &sensors {
        match collectors.get(&sensor.sensor) {
            None => notices.push(explain::sensor_offline(layout, &sensor.sensor, false)),
            Some((false, _)) => notices.push(explain::sensor_offline(layout, &sensor.sensor, true)),
            Some((true, kinds)) => notices.extend(kinds.iter().filter_map(|(kind, status)| {
                explain::collector_notice(layout, &sensor.sensor, *kind, status)
            })),
        }
    }
    notices.extend(
        engine
            .warnings()
            .iter()
            .map(|w| explain::warning(layout, w)),
    );
    notices.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.cmp(b)));

    let remaining = match engine.phase() {
        Phase::Baseline => engine.baseline_remaining_ms(),
        Phase::Watching => 0,
    };
    BuildingView {
        phase: phase_view(engine.phase(), remaining),
        rooms: layout.rooms.iter().map(|r| r.id.clone()).collect(),
        alerts,
        sensors,
        notices,
    }
}

/// 센서 모드 노트북 자신의 화면. 판정은 서버가 하므로 알림 없이 이 노트북의 수집 상태와 문제 안내만 보인다.
pub fn sensor_only_view(
    info: &crate::obs::SensorInfo,
    connected: bool,
    kinds: &BTreeMap<CollectorKind, CollectorStatus>,
) -> BuildingView {
    let layout = RoomLayout {
        rooms: vec![crate::config::Room {
            id: info.room.clone(),
            sensors: vec![crate::config::SensorSpec {
                id: info.id.clone(),
                position: info.position,
                inside_boundary_dbm: None,
                wifi_inside_boundary_dbm: None,
                place: None,
            }],
            adjacent: Vec::new(),
        }],
        storage: Vec::new(),
    };
    let entry = (connected, kinds.clone());
    let sensor = sensor_view(&layout, info.id.clone(), Some(&entry));
    let mut notices: Vec<Notice> = kinds
        .iter()
        .filter_map(|(kind, status)| explain::collector_notice(&layout, &info.id, *kind, status))
        .collect();
    notices.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.cmp(b)));
    BuildingView {
        phase: PhaseView {
            watching: connected,
            title: "센서 — 신호를 모아 시험본부 서버로 보내는 중".to_owned(),
            detail: "판정과 알림은 시험본부 서버가 합니다. 이 화면은 이 노트북의 수집 상태만 보여 줍니다.".to_owned(),
            remaining_secs: 0,
        },
        rooms: vec![info.room.clone()],
        alerts: Vec::new(),
        sensors: vec![sensor],
        notices,
    }
}

impl BuildingView {
    /// 감독관용 화면: 기술 정보(규칙 ID, 신호 세기, BSSID 같은 식별자)를 뺀다. 웹 대시보드처럼 여러 기기가 보는 곳과
    /// 개발자 모드가 아닌 화면에는 이것만 보낸다(AGENTS.md §3.3 — 식별자는 필요한 곳에만).
    pub fn without_technical(&self) -> BuildingView {
        let mut view = self.clone();
        for alert in &mut view.alerts {
            alert.explain.technical.clear();
            alert.text.clear();
        }
        for sensor in &mut view.sensors {
            for collector in &mut sensor.collectors {
                collector.technical.clear();
            }
        }
        for notice in &mut view.notices {
            notice.technical.clear();
        }
        view
    }
}
