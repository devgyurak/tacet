//! 보고서 재정렬. 서버는 여러 센서의 보고서를 네트워크가 준 순서로 받는다. 그 순서대로 판정하면 같은 관측
//! 열이어도 실행마다 결과가 달라질 수 있다(AGENTS.md §2 규칙 3). 그래서 잠시 모았다가 (시각, 센서 ID,
//! 일련번호) 순으로 내보낸다.

use std::collections::BTreeMap;

use crate::ids::SensorId;
use crate::obs::SensorReport;
use crate::time::Timestamp;

#[cfg(test)]
mod tests;

/// 재정렬 버퍼.
#[derive(Debug, Clone, Default)]
pub struct ReorderBuffer {
    delay_ms: u64,
    pending: BTreeMap<(Timestamp, SensorId, u64), SensorReport>,
}

impl ReorderBuffer {
    /// 보고서를 `delay_ms`만큼 붙잡아 두는 버퍼.
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            pending: BTreeMap::new(),
        }
    }

    /// 보고서를 넣는다. 같은 (시각, 센서, 일련번호)가 다시 오면 하나로 친다.
    pub fn push(&mut self, report: SensorReport) {
        let key = (report.obs.at(), report.sensor.clone(), report.seq);
        self.pending.insert(key, report);
    }

    /// `now`보다 `delay_ms` 이상 이전 보고서를 순서대로 꺼낸다.
    pub fn ready(&mut self, now: Timestamp) -> Vec<SensorReport> {
        let cutoff = Timestamp(now.0.saturating_sub(self.delay_ms));
        let later =
            self.pending
                .split_off(&(Timestamp(cutoff.0.saturating_add(1)), SensorId::new(""), 0));
        let ready = std::mem::replace(&mut self.pending, later);
        ready.into_values().collect()
    }

    /// 아직 붙잡고 있는 보고서 수.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// 비었는지.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}
