//! 세션 보고서. 식별자는 세션 솔트로 해시해서만 남긴다(설계 §8, AGENTS.md §2 규칙 12).
//!
//! 솔트는 난수라 core가 만들지 않는다. 앱이 세션마다 새로 만들어 넘기고 세션이 끝나면 버린다.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::device::DeviceKey;
use crate::engine::{AlertCandidate, Level};
use crate::ids::RoomId;
use crate::locate::Confidence;
use crate::rules::RuleId;
use crate::time::Timestamp;

#[cfg(test)]
mod tests;

/// 세션 솔트(32바이트).
#[derive(Clone, PartialEq, Eq)]
pub struct SessionSalt(pub [u8; 32]);

impl std::fmt::Debug for SessionSalt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 솔트가 로그에 찍히면 해시를 되돌리는 단서가 된다.
        f.write_str("SessionSalt(..)")
    }
}

/// 기기 키를 세션 솔트로 해시한 16진수 16자리.
pub fn hash_key(salt: &SessionSalt, key: &DeviceKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.0);
    // 키의 직렬 형태는 Debug 표현으로 충분하다 — 세션 안에서만 비교한다.
    hasher.update(format!("{key:?}").as_bytes());
    hex::encode(&hasher.finalize()[..8])
}

/// 보고서 한 줄.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportEntry {
    /// 시각.
    pub at: Timestamp,
    /// 해시한 기기 식별자.
    pub device: String,
    /// 의심 강도.
    pub level: Level,
    /// 점수.
    pub score: u32,
    /// 맞힌 규칙.
    pub rules: Vec<RuleId>,
    /// 추정 교실.
    pub room: Option<RoomId>,
    /// 신뢰도.
    pub confidence: Option<Confidence>,
}

impl ReportEntry {
    /// 알림 후보에서 만든다. 원래 식별자와 RSSI 근거는 남기지 않는다.
    pub fn from_candidate(salt: &SessionSalt, candidate: &AlertCandidate) -> Self {
        Self {
            at: candidate.at,
            device: hash_key(salt, &candidate.device),
            level: candidate.level,
            score: candidate.score,
            rules: candidate.rules.clone(),
            room: candidate.localization.as_ref().and_then(|l| l.room.clone()),
            confidence: candidate.localization.as_ref().map(|l| l.confidence),
        }
    }
}

/// 세션 보고서.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SessionReport {
    /// 항목(시각 순).
    pub entries: Vec<ReportEntry>,
}

impl SessionReport {
    /// 알림 후보를 더한다.
    pub fn push(&mut self, salt: &SessionSalt, candidate: &AlertCandidate) {
        self.entries
            .push(ReportEntry::from_candidate(salt, candidate));
    }
}
