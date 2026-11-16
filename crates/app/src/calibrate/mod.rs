//! 교실 보정 모드(`tacet calibrate`, `docs/rules/L-room-calibration.md`).
//!
//! 블루투스·와이파이 수집기만 띄우고 관측을 core의 [`Calibrator`]에 넣는다. 화면(Electron)이 보내는 "고르기"·"교실 끝"·
//! "다시" 요청으로 단계를 넘기고, 진행 상황을 JSON 줄로 보낸다. 판정은 core가 한다 — 여기는 잇기만 한다.

use std::io::Write;
use std::time::Duration;

use tacet_core::calibrate::Calibrator;
use tacet_core::explain;
use tacet_core::ids::SensorId;
use tacet_core::obs::{CollectorEvent, CollectorKind, CollectorStatus};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;

use crate::ipc::{Command, Outgoing, encode};

#[cfg(test)]
mod tests;

/// 진행 상황을 확인하는 간격.
const TICK: Duration = Duration::from_millis(500);

/// 보정을 돌린다. 화면이 끝내기를 요청하거나 요청 통로가 닫히면 끝난다. 출력이 끊기면 오류로 끝난다.
pub async fn run(
    mut events: mpsc::Receiver<CollectorEvent>,
    mut commands: mpsc::Receiver<Command>,
    clock: ClockFn,
    out: &mut impl Write,
) -> std::io::Result<()> {
    // 보정은 이 노트북 한 대의 값이라 센서 이름은 쓰이지 않는다(기기 키에만 들어간다).
    let sensor = SensorId::new("이 노트북");
    let mut calibrator = Calibrator::default();
    let mut bluetooth_problem: Option<String> = None;
    let mut wifi_problem: Option<String> = None;
    let mut last: Option<Outgoing> = None;
    let mut events_open = true;
    let mut tick = tokio::time::interval(TICK);
    loop {
        // 관측 한 건마다 목록 전체를 다시 만들지 않는다 — 진행 상황은 시계 틱과 요청 뒤에만 낸다.
        let mut report = true;
        tokio::select! {
            event = events.recv(), if events_open => match event {
                Some(CollectorEvent::Obs(obs)) => {
                    calibrator.observe(&sensor, &obs);
                    report = false;
                }
                Some(CollectorEvent::Status(kind @ (CollectorKind::Ble | CollectorKind::Wifi), status)) => {
                    let problem = match &status {
                        CollectorStatus::Off(_) => Some(explain::collector_state(kind, &status).1),
                        _ => None,
                    };
                    if kind == CollectorKind::Ble {
                        bluetooth_problem = problem;
                    } else {
                        wifi_problem = problem;
                    }
                }
                Some(CollectorEvent::Status(..)) => {}
                None => {
                    // 수집기가 모두 끝났다 — 조용히 넘어가면 감독관은 휴대폰 탓을 한다(AGENTS.md §2 규칙 5).
                    events_open = false;
                    let stopped = "신호 감지가 멈췄습니다. 앱을 다시 시작해 주세요.";
                    bluetooth_problem.get_or_insert_with(|| stopped.to_owned());
                    wifi_problem.get_or_insert_with(|| stopped.to_owned());
                }
            },
            command = commands.recv() => match command {
                Some(Command::CalibrateSelect { bluetooth, wifi }) => {
                    // 실패하면 그 이유가 진행 상황에 실린다.
                    let _ = calibrator.select(&bluetooth, &wifi, clock());
                }
                Some(Command::CalibrateFar) => {
                    // 고르기 전이면 무시한다. 화면은 고른 뒤에만 이 단추를 보인다.
                    let _ = calibrator.start_far(clock());
                }
                Some(Command::CalibrateRestart) => calibrator.restart(),
                Some(Command::OpenLocationSettings) => crate::platform::open_location_settings(),
                Some(Command::Shutdown) | None => return Ok(()),
                Some(_) => {}
            },
            _ = tick.tick() => {}
        }
        if !report {
            continue;
        }
        let now = clock();
        calibrator.tick(now);
        let message = Outgoing::Calibration {
            progress: Box::new(calibrator.progress(now)),
            bluetooth_problem: bluetooth_problem.clone(),
            wifi_problem: wifi_problem.clone(),
        };
        if last.as_ref() != Some(&message) {
            writeln!(out, "{}", encode(&message))?;
            out.flush()?;
            last = Some(message);
        }
    }
}
