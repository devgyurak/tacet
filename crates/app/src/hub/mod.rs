//! 판정 허브: 엔진, 센서별 수집기 상태, 세션 보고서. 단독 모드와 서버 모드가 같이 쓴다.
//!
//! 판정은 core 엔진이 한다. 허브는 사건을 엔진에 넣고 화면 모델을 만들 뿐이다.

use std::collections::BTreeMap;

use tacet_core::config::{DetectorConfig, RoomLayout, Whitelist};
use tacet_core::engine::{AlertCandidate, Engine};
use tacet_core::ids::SensorId;
use tacet_core::obs::{CollectorKind, CollectorStatus, Observation, SensorReport};
use tacet_core::order::ReorderBuffer;
use tacet_core::report::{SessionReport, SessionSalt};
use tacet_core::text;
use tacet_core::time::Timestamp;
use tacet_core::view::{BuildingView, CollectorTable, building_view};

#[cfg(test)]
mod tests;

/// 판정 허브.
#[derive(Debug)]
pub struct Hub {
    engine: Engine,
    collectors: CollectorTable,
    salt: SessionSalt,
    report: SessionReport,
    seq: BTreeMap<SensorId, u64>,
    log: Vec<String>,
    pending: ReorderBuffer,
}

/// 알림 기록에 남기는 최대 줄 수.
const LOG_LIMIT: usize = 200;
/// 서버 모드에서 센서 보고서를 붙잡아 두는 시간(ms). 이만큼 늦게 온 보고서까지 (시각, 센서, 일련번호) 순으로
/// 판정해 실행마다 같은 결과를 낸다(AGENTS.md §2 규칙 3).
pub const REORDER_DELAY_MS: u64 = 2_000;

impl Hub {
    /// 세션을 시작한다. 솔트는 앱이 세션마다 새로 만든다.
    pub fn new(
        cfg: DetectorConfig,
        layout: RoomLayout,
        whitelist: Whitelist,
        salt: SessionSalt,
    ) -> Self {
        Self {
            engine: Engine::new(cfg, layout, whitelist),
            collectors: BTreeMap::new(),
            salt,
            report: SessionReport::default(),
            seq: BTreeMap::new(),
            log: Vec::new(),
            pending: ReorderBuffer::new(REORDER_DELAY_MS),
        }
    }

    /// 센서가 연결됐는지 표시한다.
    pub fn set_connected(&mut self, sensor: &SensorId, connected: bool) {
        self.collectors.entry(sensor.clone()).or_default().0 = connected;
    }

    /// 수집기 상태를 바꾼다.
    pub fn set_status(&mut self, sensor: &SensorId, kind: CollectorKind, status: CollectorStatus) {
        let entry = self.collectors.entry(sensor.clone()).or_default();
        entry.0 = true;
        entry.1.insert(kind, status);
    }

    /// 센서 보고서를 엔진에 넣는다. 새 알림은 기록과 보고서에 남긴다.
    pub fn ingest(&mut self, report: &SensorReport) -> Vec<AlertCandidate> {
        let alerts = self.engine.ingest(report);
        for alert in &alerts {
            self.report.push(&self.salt, alert);
            let first_line = text::alert(alert, self.engine.layout())
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned();
            self.log.push(first_line);
        }
        if self.log.len() > LOG_LIMIT {
            let overflow = self.log.len() - LOG_LIMIT;
            self.log.drain(..overflow);
        }
        alerts
    }

    /// 이 컴퓨터의 수집기가 낸 관측을 보고서로 만들어 넣는다(단독 모드).
    pub fn ingest_local(&mut self, sensor: &SensorId, obs: Observation) -> Vec<AlertCandidate> {
        let seq = self.seq.entry(sensor.clone()).or_default();
        *seq += 1;
        let report = SensorReport {
            sensor: sensor.clone(),
            seq: *seq,
            obs,
        };
        self.ingest(&report)
    }

    /// 네트워크로 받은 센서 보고서를 재정렬 버퍼에 넣는다(서버 모드). 판정은 [`Hub::tick`]에서 한다.
    pub fn receive(&mut self, report: SensorReport) {
        self.pending.push(report);
    }

    /// 재정렬 버퍼에서 판정할 차례가 된 보고서를 넣고, 시각을 앞으로 옮긴다(기준선 종료).
    pub fn tick(&mut self, now: Timestamp) -> Vec<AlertCandidate> {
        let ready = self.pending.ready(now);
        let alerts = ready
            .iter()
            .flat_map(|report| self.ingest(report))
            .collect();
        self.engine.tick(now);
        alerts
    }

    /// 화면 모델.
    pub fn view(&self) -> BuildingView {
        building_view(&self.engine, &self.collectors)
    }

    /// 최근 알림 첫 줄들(오래된 것부터).
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// 세션 보고서(해시한 식별자만).
    pub fn report(&self) -> &SessionReport {
        &self.report
    }
}
