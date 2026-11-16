//! 클라이언트 쪽: 센서와 감독관 뷰가 서버에 붙는다.

use std::net::SocketAddr;
use std::time::Duration;

use rustls::pki_types::ServerName;
use tacet_core::obs::SensorInfo;
use tacet_core::view::BuildingView;
use tokio::io::{BufReader, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use crate::NetError;
use crate::frame::{read_frame, write_frame};
use crate::pairing::{Fingerprint, proof};
use crate::protocol::{ClientHello, Downlink, Role, ServerHello, SessionInfo, Uplink, VERSION};
use crate::tls::{SERVER_NAME, client_config};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

type Reader = BufReader<ReadHalf<TlsStream<TcpStream>>>;
type Writer = WriteHalf<TlsStream<TcpStream>>;

async fn connect(
    addr: SocketAddr,
    code: &str,
    pinned: Option<Fingerprint>,
    role: Role,
) -> Result<(Reader, Writer, SessionInfo, Fingerprint), NetError> {
    let (config, verifier) = client_config(pinned)?;
    let connector = TlsConnector::from(config);
    let name = ServerName::try_from(SERVER_NAME).map_err(|e| NetError::Tls(e.to_string()))?;
    let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| NetError::Timeout)??;
    let tls = match tokio::time::timeout(CONNECT_TIMEOUT, connector.connect(name, tcp))
        .await
        .map_err(|_| NetError::Timeout)?
    {
        Ok(tls) => tls,
        Err(_) if pinned.is_some() && verifier.seen() != pinned => {
            return Err(NetError::PinMismatch);
        }
        Err(e) => return Err(NetError::Io(e)),
    };
    let fingerprint = verifier
        .seen()
        .ok_or_else(|| NetError::Tls("서버 인증서를 보지 못했다".to_owned()))?;
    let (read_half, mut writer) = tokio::io::split(tls);
    let mut reader = BufReader::new(read_half);
    let hello = ClientHello {
        version: VERSION,
        proof: proof(code, &fingerprint, &role.tag()),
        role,
    };
    write_frame(&mut writer, &hello).await?;
    let answer: ServerHello = tokio::time::timeout(CONNECT_TIMEOUT, read_frame(&mut reader))
        .await
        .map_err(|_| NetError::Timeout)??
        .ok_or(NetError::Closed)?;
    match answer {
        ServerHello::Accepted(session) => Ok((reader, writer, session, fingerprint)),
        ServerHello::Rejected(reason) => Err(NetError::Rejected(reason)),
    }
}

/// 센서 연결.
#[derive(Debug)]
pub struct SensorLink {
    writer: Writer,
    _reader: Reader,
}

impl SensorLink {
    /// 보고서나 상태를 보낸다.
    pub async fn send(&mut self, uplink: &Uplink) -> Result<(), NetError> {
        write_frame(&mut self.writer, uplink).await
    }
}

/// 센서로 서버에 붙는다. 처음이면 `pinned`를 비우고, 돌려받은 지문을 저장해 다음부터 고정한다.
pub async fn connect_sensor(
    addr: SocketAddr,
    code: &str,
    pinned: Option<Fingerprint>,
    info: SensorInfo,
) -> Result<(SensorLink, SessionInfo, Fingerprint), NetError> {
    let (reader, writer, session, fingerprint) =
        connect(addr, code, pinned, Role::Sensor(info)).await?;
    Ok((
        SensorLink {
            writer,
            _reader: reader,
        },
        session,
        fingerprint,
    ))
}

/// 감독관 뷰 연결.
#[derive(Debug)]
pub struct ViewerLink {
    reader: Reader,
    _writer: Writer,
}

impl ViewerLink {
    /// 다음 화면. 연결이 끝나면 `None`.
    pub async fn next_view(&mut self) -> Result<Option<BuildingView>, NetError> {
        Ok(read_frame::<_, Downlink>(&mut self.reader)
            .await?
            .map(|Downlink::View(view)| view))
    }
}

/// 감독관 뷰로 서버에 붙는다.
pub async fn connect_viewer(
    addr: SocketAddr,
    code: &str,
    pinned: Option<Fingerprint>,
) -> Result<(ViewerLink, Fingerprint), NetError> {
    let (reader, writer, _, fingerprint) = connect(addr, code, pinned, Role::Viewer).await?;
    Ok((
        ViewerLink {
            reader,
            _writer: writer,
        },
        fingerprint,
    ))
}
