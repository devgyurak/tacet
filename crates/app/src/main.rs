//! 앱 백엔드 진입점. 화면은 Electron(`desktop/`)이 이 바이너리를 자식 프로세스로 띄워 그린다.
//!
//! ```text
//! tacet --config <설정.toml>   표준 출력으로 화면 모델 JSON 줄을 보내고, 표준 입력으로 요청을 받는다
//! tacet discover               시험장 네트워크의 서버를 찾아 JSON으로 출력한다(개발자 모드)
//! tacet calibrate              블루투스·와이파이만 켜고 교실 보정 진행 상황을 JSON 줄로 보낸다(docs/rules/L-room-calibration.md)
//! ```

use std::io::Write as _;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use tacet::config::{AppConfig, Mode, config_path_from_args};
use tacet::ipc::{self, Outgoing};
use tacet::{platform, runtime};

/// 화면 모델을 확인하는 간격.
const PUMP_INTERVAL: Duration = Duration::from_millis(300);

fn load(path: &std::path::Path) -> anyhow::Result<AppConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("설정 파일을 읽지 못했다: {}", path.display()))?;
    AppConfig::parse(&text)
        .with_context(|| format!("설정 파일이 올바르지 않다: {}", path.display()))
}

/// 시작하지 못한 이유를 화면(표준 출력 JSON)과 사람(표준 오류) 모두에게 알린다.
fn fail(error: &anyhow::Error) -> ExitCode {
    let message = format!("{error:#}");
    println!(
        "{}",
        ipc::encode(&Outgoing::Error {
            message: message.clone()
        })
    );
    eprintln!("오류: {message}");
    ExitCode::FAILURE
}

fn discover() -> ExitCode {
    match tacet_net::discovery::discover(Duration::from_secs(3)) {
        Ok(found) => {
            let list: Vec<serde_json::Value> = found
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.name,
                        "address": f.addr.to_string(),
                        "fingerprint_short": f.fingerprint_short,
                    })
                })
                .collect();
            println!("{}", serde_json::Value::Array(list));
            ExitCode::SUCCESS
        }
        Err(e) => fail(&anyhow::anyhow!("서버를 찾지 못했다: {e}")),
    }
}

/// 교실 보정 모드: 블루투스·와이파이 수집기만 띄우고, 표준 입력의 요청으로 단계를 넘기며, 진행 상황을 표준 출력으로 보낸다.
fn calibrate() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => return fail(&anyhow::anyhow!("비동기 런타임을 만들지 못했다: {e}")),
    };
    let started = std::time::Instant::now();
    let clock: tacet_core::time::ClockFn = Arc::new(move || {
        tacet_core::time::Timestamp(
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        )
    });
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(1024);
    let (command_tx, command_rx) = tokio::sync::mpsc::channel(16);
    runtime.spawn(tacet_collector_ble::run_for_calibration(
        clock.clone(),
        event_tx.clone(),
    ));
    runtime.spawn(tacet_collector_wifi::run(clock.clone(), event_tx));
    // 와이파이 이름·주소를 받으려면 macOS 위치 권한이 필요하다. 권한 요청은 메인 스레드에서 한다.
    tacet_collector_wifi::request_location_permission();
    std::thread::spawn(move || {
        use std::io::BufRead as _;
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { continue };
            if let Some(command) = ipc::parse_command(&line)
                && command_tx.blocking_send(command).is_err()
            {
                return;
            }
        }
        // 입력이 닫히면 요청 통로도 닫혀 보정이 끝난다.
    });
    std::thread::spawn(move || {
        let mut out = std::io::stdout().lock();
        let _ = runtime.block_on(tacet::calibrate::run(event_rx, command_rx, clock, &mut out));
        let _ = out.flush();
        std::process::exit(0);
    });
    platform::park_main_thread()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("discover") => return discover(),
        Some("calibrate") => return calibrate(),
        _ => {}
    }
    let path = config_path_from_args(args, platform::state_dir().join("tacet.toml"));
    let config = match load(&path) {
        Ok(config) => config,
        Err(e) => return fail(&e),
    };
    let state_dir = config.state_dir();
    let _power = platform::prevent_sleep();
    let handles = match runtime::start(config.clone()) {
        Ok(handles) => Arc::new(handles),
        Err(e) => return fail(&e),
    };
    // 위치 권한 요청은 메인 스레드에서 해야 한다(CoreLocation). Wi-Fi를 수집하는 모드만 요청한다.
    if matches!(config.mode, Mode::Standalone | Mode::Sensor) {
        tacet_collector_wifi::request_location_permission();
    }

    let pump_handles = handles.clone();
    std::thread::spawn(move || {
        let mut last = None;
        let mut out = std::io::stdout().lock();
        loop {
            // 화면이 닫혀 출력 파이프가 끊기면 함께 끝난다.
            if ipc::pump(&pump_handles.model, &mut last, &mut out).is_err() {
                let _ = out.flush();
                std::process::exit(0);
            }
            std::thread::sleep(PUMP_INTERVAL);
        }
    });
    std::thread::spawn(move || {
        ipc::serve_commands(std::io::stdin().lock(), &handles, &state_dir);
        // 화면이 끝내기를 요청했거나 입력이 닫혔다(화면 프로세스가 죽은 경우 포함).
        std::process::exit(0);
    });
    platform::park_main_thread()
}
