//! `tacet-devtools <녹화|재생>`.
//!
//! - `record <설정.toml> <출력.obslog> <초>`: 이 컴퓨터의 수집기 관측을 녹화한다(저장소 밖에만).
//! - `replay <설정.toml> <입력.obslog>`: 녹화를 판정하고 분해해 보인다(식별자는 해시).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use rand::RngCore;
use tacet_core::ids::SensorId;
use tacet_core::obs::{CollectorEvent, SensorReport};
use tacet_core::report::SessionSalt;
use tacet_core::time::{ClockFn, Timestamp};
use tacet_devtools::{Command, check_record_path, obslog_line, parse_command, run_replay};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

async fn record(out: PathBuf, seconds: u64) -> anyhow::Result<()> {
    check_record_path(&out).map_err(anyhow::Error::msg)?;
    let started = Instant::now();
    let clock: ClockFn = Arc::new(move || {
        Timestamp(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
    });
    let (tx, mut rx) = mpsc::channel(4096);
    tokio::spawn(tacet_collector_ble::run(clock.clone(), tx.clone()));
    tokio::spawn(tacet_collector_wifi::run(clock.clone(), tx.clone()));
    drop(tx);
    // 녹화는 남의 무선 식별자를 담는다 — 이 사용자만 읽을 수 있게(유닉스 0600) 처음부터 그 권한으로 만든다.
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&out).await?;
    // 이미 있던 파일은 만들 때의 권한이 적용되지 않으니 다시 좁힌다.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .await?;
    }
    let sensor = SensorId::new("녹화");
    let deadline = tokio::time::sleep(Duration::from_secs(seconds));
    tokio::pin!(deadline);
    let mut seq = 0u64;
    let mut written = 0usize;
    loop {
        tokio::select! {
            () = &mut deadline => break,
            event = rx.recv() => match event {
                Some(CollectorEvent::Obs(obs)) => {
                    seq += 1;
                    let line = obslog_line(&SensorReport { sensor: sensor.clone(), seq, obs });
                    file.write_all(format!("{line}\n").as_bytes()).await?;
                    written += 1;
                }
                Some(CollectorEvent::Status(kind, status)) => eprintln!("{}", tacet_core::text::collector_status(kind, &status)),
                None => break,
            },
        }
    }
    // 끝나기 전에 디스크까지 내려 쓴다(중간에 끊겨도 쓴 만큼은 남게).
    file.flush().await?;
    file.sync_all().await?;
    eprintln!("관측 {written}건을 녹화했다: {}", out.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_command(&args).map_err(anyhow::Error::msg)? {
        Command::Record { out, seconds } => {
            tokio::runtime::Runtime::new()?.block_on(record(out, seconds))
        }
        Command::Replay { config, input } => {
            let config = std::fs::read_to_string(&config)
                .with_context(|| format!("설정을 읽지 못했다: {}", config.display()))?;
            let obslog = std::fs::read_to_string(&input)?;
            let mut salt = [0u8; 32];
            rand::rng().fill_bytes(&mut salt);
            println!(
                "{}",
                run_replay(&config, &obslog, &SessionSalt(salt)).map_err(anyhow::Error::msg)?
            );
            Ok(())
        }
    }
}
