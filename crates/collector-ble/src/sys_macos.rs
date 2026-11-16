//! macOS 어댑터(btleplug → CoreBluetooth). 능동 스캔만 되며 허용한 예외다(설계 §7.2).

use btleplug::api::{
    Central, CentralEvent, CentralState, Manager as _, Peripheral as _, ScanFilter,
};
use btleplug::platform::{Adapter, Manager, PeripheralId};
use futures::StreamExt;
use tacet_core::obs::{
    CollectorEvent, CollectorKind, CollectorStatus, Observation, OffReason, Permission,
};
use tacet_core::time::ClockFn;
use tokio::sync::mpsc;

use crate::adv::{RawAdvertisement, apply_names, to_obs, uuid_from_text};

async fn status(tx: &mpsc::Sender<CollectorEvent>, status: CollectorStatus) {
    let _ = tx
        .send(CollectorEvent::Status(CollectorKind::Ble, status))
        .await;
}

fn off_reason(error: &btleplug::Error) -> OffReason {
    match error {
        btleplug::Error::PermissionDenied => OffReason::PermissionDenied(Permission::Bluetooth),
        other => OffReason::Error(other.to_string()),
    }
}

async fn advertisement(adapter: &Adapter, id: &PeripheralId) -> Option<RawAdvertisement> {
    let peripheral = adapter.peripheral(id).await.ok()?;
    let props = peripheral.properties().await.ok()??;
    Some(RawAdvertisement {
        addr: Some(props.address.into_inner()),
        host_uuid: uuid_from_text(&id.to_string()),
        rssi: props.rssi,
        manufacturer: props.manufacturer_data.into_iter().collect(),
        services: props.services.iter().map(uuid::Uuid::as_u128).collect(),
        name: props.local_name.map(String::into_bytes),
        service_data: props
            .service_data
            .into_iter()
            .map(|(u, d)| (u.as_u128(), d))
            .collect(),
    })
}

pub(crate) async fn run(clock: ClockFn, tx: mpsc::Sender<CollectorEvent>, names: crate::Names) {
    let manager = match Manager::new().await {
        Ok(m) => m,
        Err(e) => return status(&tx, CollectorStatus::Off(off_reason(&e))).await,
    };
    let Some(adapter) = manager
        .adapters()
        .await
        .ok()
        .and_then(|a| a.into_iter().next())
    else {
        return status(&tx, CollectorStatus::Off(OffReason::DeviceMissing)).await;
    };
    let mut events = match adapter.events().await {
        Ok(events) => events,
        Err(e) => return status(&tx, CollectorStatus::Off(off_reason(&e))).await,
    };
    if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
        return status(&tx, CollectorStatus::Off(off_reason(&e))).await;
    }
    status(&tx, CollectorStatus::Running).await;
    while let Some(event) = events.next().await {
        let id = match event {
            CentralEvent::DeviceDiscovered(id)
            | CentralEvent::DeviceUpdated(id)
            | CentralEvent::ManufacturerDataAdvertisement { id, .. }
            | CentralEvent::ServiceDataAdvertisement { id, .. }
            | CentralEvent::ServicesAdvertisement { id, .. } => id,
            CentralEvent::StateUpdate(CentralState::PoweredOn) => {
                status(&tx, CollectorStatus::Running).await;
                continue;
            }
            CentralEvent::StateUpdate(_) => {
                status(
                    &tx,
                    CollectorStatus::Off(OffReason::Error("블루투스가 꺼져 있다".to_owned())),
                )
                .await;
                continue;
            }
            _ => continue,
        };
        let Some(obs) = advertisement(&adapter, &id)
            .await
            .and_then(|raw| to_obs(raw, clock()))
            .map(|obs| apply_names(obs, names))
        else {
            continue;
        };
        if tx
            .send(CollectorEvent::Obs(Observation::Ble(obs)))
            .await
            .is_err()
        {
            let _ = adapter.stop_scan().await;
            return;
        }
    }
    // 사건 흐름이 끝났다 — 조용히 멈추지 않고 꺼짐을 알린다.
    status(
        &tx,
        CollectorStatus::Off(OffReason::Error("블루투스 사건 흐름이 끊겼다".to_owned())),
    )
    .await;
}
