//! 개발 도구(설계 §5.3 devtools). 배포 앱에 들어가지 않는다.
//!
//! - 녹화: 이 컴퓨터의 수집기 관측을 `.obslog`(JSON 줄)로 저장한다. **저장소 안에는 쓰지 않는다** — 실제
//!   식별자가 들어 있다(AGENTS.md §3.3). 녹화 파일을 대화로 읽지 않는다.
//! - 재생: `.obslog`를 core 엔진에 넣고 판정을 분해해 보인다. 식별자는 세션 솔트 해시로만 찍는다.

use std::path::Path;

use serde::Deserialize;
use tacet_core::config::{DetectorConfig, RoomLayout, Whitelist};
use tacet_core::engine::Engine;
use tacet_core::obs::SensorReport;
use tacet_core::report::{SessionSalt, hash_key};
use tacet_core::text;

#[cfg(test)]
mod tests;

/// 녹화 파일 확장자. 캡처와 똑같이 다룬다(추적·대화로 읽기 금지).
pub const OBSLOG_EXT: &str = "obslog";

/// 재생 설정(TOML): 앱 설정에서 판정 부분만.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReplayConfig {
    /// 판정 설정.
    #[serde(default)]
    pub detector: DetectorConfig,
    /// 배치.
    #[serde(default)]
    pub layout: RoomLayout,
    /// 화이트리스트.
    #[serde(default)]
    pub whitelist: Whitelist,
}

/// 녹화 파일을 읽는다. (보고서들, 버린 줄 수).
pub fn parse_obslog(text: &str) -> (Vec<SensorReport>, usize) {
    let mut reports = Vec::new();
    let mut dropped = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<SensorReport>(line) {
            Ok(report) => reports.push(report),
            Err(_) => dropped += 1,
        }
    }
    (reports, dropped)
}

/// 보고서 하나를 녹화 한 줄로.
pub fn obslog_line(report: &SensorReport) -> String {
    serde_json::to_string(report).unwrap_or_default()
}

/// 재생 결과 한 줄(식별자는 해시).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayLine {
    /// 시각(초).
    pub at_secs: u64,
    /// 해시한 기기.
    pub device: String,
    /// 문구(식별자를 뺀 요약).
    pub summary: String,
}

/// 녹화를 엔진에 넣어 알림마다 분해 줄을 만든다.
pub fn replay(
    config: &ReplayConfig,
    reports: &[SensorReport],
    salt: &SessionSalt,
) -> Vec<ReplayLine> {
    let mut engine = Engine::new(
        config.detector.clone(),
        config.layout.clone(),
        config.whitelist.clone(),
    );
    let mut lines = Vec::new();
    for report in reports {
        for alert in engine.ingest(report) {
            let rules: Vec<&str> = alert.rules.iter().map(|r| text::rule(*r)).collect();
            let place = alert
                .localization
                .as_ref()
                .map_or_else(|| "세기 정보 없음".to_owned(), text::location);
            let evidence = alert
                .localization
                .as_ref()
                .map(|l| {
                    l.evidence
                        .iter()
                        .map(|(s, v)| format!("{s} {v}"))
                        .collect::<Vec<_>>()
                        .join(" / ")
                })
                .unwrap_or_default();
            lines.push(ReplayLine {
                at_secs: alert.at.0 / 1000,
                device: hash_key(salt, &alert.device),
                summary: format!(
                    "의심 강도 {} (점수 {}) · {} · {} · 근거 {}",
                    text::level(alert.level),
                    alert.score,
                    rules.join(", "),
                    place,
                    evidence
                ),
            });
        }
    }
    lines
}

/// 경로가 git 저장소 안인지(조상 폴더에 `.git`이 있는지). 녹화는 저장소 밖에만 쓴다.
pub fn inside_git_repo(path: &Path) -> bool {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    absolute
        .ancestors()
        .skip(1)
        .any(|dir| dir.join(".git").exists())
}

/// 녹화 출력 경로를 확인한다.
pub fn check_record_path(path: &Path) -> Result<(), String> {
    if path.extension().and_then(|e| e.to_str()) != Some(OBSLOG_EXT) {
        return Err(format!("녹화 파일은 .{OBSLOG_EXT} 확장자여야 한다"));
    }
    if inside_git_repo(path) {
        return Err("녹화 파일은 git 저장소 밖에 쓴다 — 실제 식별자가 들어 있다".to_owned());
    }
    Ok(())
}

/// 사용법 문구.
pub const USAGE: &str = "사용법: tacet-devtools record <설정.toml> <출력.obslog> <초> | replay <설정.toml> <입력.obslog>";

/// 명령줄 해석 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 녹화.
    Record {
        /// 출력 경로.
        out: std::path::PathBuf,
        /// 녹화 시간(초).
        seconds: u64,
    },
    /// 재생.
    Replay {
        /// 설정 경로.
        config: std::path::PathBuf,
        /// 입력 녹화.
        input: std::path::PathBuf,
    },
}

/// 명령줄을 해석한다. 녹화 경로는 여기서 확인한다(장비를 열기 전에).
pub fn parse_command(args: &[String]) -> Result<Command, String> {
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["record", _config, out, seconds] => {
            let seconds = seconds.parse().map_err(|_| "초는 정수다".to_owned())?;
            let out = std::path::PathBuf::from(out);
            check_record_path(&out)?;
            Ok(Command::Record { out, seconds })
        }
        ["replay", config, input] => Ok(Command::Replay {
            config: config.into(),
            input: input.into(),
        }),
        _ => Err(USAGE.to_owned()),
    }
}

/// 재생 명령을 실행해 출력 문구를 돌려준다.
pub fn run_replay(
    config_text: &str,
    obslog_text: &str,
    salt: &SessionSalt,
) -> Result<String, String> {
    let config: ReplayConfig =
        toml::from_str(config_text).map_err(|e| format!("설정 형식이 틀렸다: {e}"))?;
    let (reports, dropped) = parse_obslog(obslog_text);
    let mut out: Vec<String> = replay(&config, &reports, salt)
        .iter()
        .map(|l| format!("{:>6}초 {} {}", l.at_secs, l.device, l.summary))
        .collect();
    out.push(format!(
        "보고서 {}건, 형식이 틀린 줄 {dropped}건",
        reports.len()
    ));
    Ok(out.join("\n"))
}
