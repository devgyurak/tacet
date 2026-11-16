//! Windows 어댑터: WinRT 광고 감시기를 수동(passive) 스캔으로 쓴다(AGENTS.md §3.1).

use tacet_core::obs::{
    CollectorEvent, CollectorKind, CollectorStatus, Observation, OffReason, Permission,
};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;
use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
    BluetoothLEAdvertisementWatcherStoppedEventArgs, BluetoothLEScanningMode,
};
use windows::Devices::Bluetooth::BluetoothError;
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::{DataReader, IBuffer};

use crate::adv::{RawAdvertisement, addr_from_u64, apply_names, apply_section, to_obs};

fn bytes(buffer: &IBuffer) -> windows::core::Result<Vec<u8>> {
    let reader = DataReader::FromBuffer(buffer)?;
    let mut out = vec![0u8; reader.UnconsumedBufferLength()? as usize];
    reader.ReadBytes(&mut out)?;
    Ok(out)
}

fn read_advertisement(
    args: &BluetoothLEAdvertisementReceivedEventArgs,
) -> windows::core::Result<RawAdvertisement> {
    let mut raw = RawAdvertisement {
        addr: Some(addr_from_u64(args.BluetoothAddress()?)),
        rssi: Some(args.RawSignalStrengthInDBm()?),
        ..RawAdvertisement::default()
    };
    let adv = args.Advertisement()?;
    let sections = adv.DataSections()?;
    for i in 0..sections.Size()? {
        let section = sections.GetAt(i)?;
        apply_section(&mut raw, section.DataType()?, &bytes(&section.Data()?)?);
    }
    Ok(raw)
}

fn stop_reason(error: BluetoothError) -> OffReason {
    if error == BluetoothError::DisabledByUser || error == BluetoothError::ConsentRequired {
        OffReason::PermissionDenied(Permission::Bluetooth)
    } else if error == BluetoothError::RadioNotAvailable {
        OffReason::DeviceMissing
    } else {
        OffReason::Error(format!("광고 감시가 멈췄다({})", error.0))
    }
}

pub(crate) async fn run(clock: ClockFn, tx: mpsc::Sender<CollectorEvent>, names: crate::Names) {
    let setup = || -> windows::core::Result<BluetoothLEAdvertisementWatcher> {
        let watcher = BluetoothLEAdvertisementWatcher::new()?;
        watcher.SetScanningMode(BluetoothLEScanningMode::Passive)?;
        let obs_tx = tx.clone();
        watcher.Received(&TypedEventHandler::<
            BluetoothLEAdvertisementWatcher,
            BluetoothLEAdvertisementReceivedEventArgs,
        >::new(move |_, args| {
            let obs = args
                .as_ref()
                .and_then(|a| read_advertisement(a).ok())
                .and_then(|raw| to_obs(raw, clock()))
                .map(|obs| apply_names(obs, names));
            if let Some(obs) = obs {
                // 감시기 스레드를 막지 않는다. 받는 쪽이 밀리면 이 광고는 버린다.
                let _ = obs_tx.try_send(CollectorEvent::Obs(Observation::Ble(obs)));
            }
            Ok(())
        }))?;
        let stop_tx = tx.clone();
        watcher.Stopped(&TypedEventHandler::<
            BluetoothLEAdvertisementWatcher,
            BluetoothLEAdvertisementWatcherStoppedEventArgs,
        >::new(move |_, args| {
            let reason = args.as_ref().and_then(|a| a.Error().ok()).map_or(
                OffReason::Error("광고 감시가 멈췄다".to_owned()),
                stop_reason,
            );
            // 꺼짐은 탐지 공백이라 버리면 안 된다. 이 콜백은 WinRT 스레드에서 돌아 tokio 런타임 밖이므로
            // 기다려서 보내도 된다(받는 쪽이 닫혔으면 바로 끝난다).
            let _ = stop_tx.blocking_send(CollectorEvent::Status(
                CollectorKind::Ble,
                CollectorStatus::Off(reason),
            ));
            Ok(())
        }))?;
        watcher.Start()?;
        Ok(watcher)
    };
    match setup() {
        Ok(watcher) => {
            let _ = tx
                .send(CollectorEvent::Status(
                    CollectorKind::Ble,
                    CollectorStatus::Running,
                ))
                .await;
            tx.closed().await;
            let _ = watcher.Stop();
        }
        Err(e) => {
            let reason = OffReason::Error(e.message().to_string());
            let _ = tx
                .send(CollectorEvent::Status(
                    CollectorKind::Ble,
                    CollectorStatus::Off(reason),
                ))
                .await;
        }
    }
}
