//! TLS: 서버는 자체 서명 인증서를 쓰고, 클라이언트는 그 지문을 고정한다.
//!
//! 공인 인증서가 없는 내부망이라 CA 검증 대신 지문 고정과 페어링 증명으로 서버를 확인한다
//! ([`crate::pairing`]). 핸드셰이크 서명 검증은 그대로 해서, 서버가 인증서의 개인키를 가진 것은 확인한다.

use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, ring, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

use crate::NetError;
use crate::pairing::Fingerprint;

#[cfg(test)]
mod tests;

/// TLS에 쓰는 서버 이름. 인증서 이름 검증은 하지 않고 지문으로 확인한다.
pub const SERVER_NAME: &str = "tacet.local";

fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}

/// 서버 인증서와 키. 서버를 켤 때마다 새로 만들거나, 고정을 유지하려면 앱이 저장해 둔다.
#[derive(Clone)]
pub struct ServerIdentity {
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
}

impl std::fmt::Debug for ServerIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 개인키를 로그에 찍지 않는다.
        write!(f, "ServerIdentity({})", self.fingerprint().short())
    }
}

impl ServerIdentity {
    /// 새 자체 서명 인증서를 만든다.
    pub fn generate() -> Result<Self, NetError> {
        let certified = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_owned()])
            .map_err(|e| NetError::Tls(e.to_string()))?;
        Ok(Self {
            cert_der: certified.cert.der().to_vec(),
            key_der: certified.signing_key.serialize_der(),
        })
    }

    /// 저장해 둔 인증서·키(DER)로 만든다.
    pub fn from_der(cert_der: Vec<u8>, key_der: Vec<u8>) -> Self {
        Self { cert_der, key_der }
    }

    /// 인증서 DER.
    pub fn cert_der(&self) -> &[u8] {
        &self.cert_der
    }

    /// 개인키 DER(PKCS#8).
    pub fn key_der(&self) -> &[u8] {
        &self.key_der
    }

    /// 인증서 지문.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_der(&self.cert_der)
    }

    /// rustls 서버 설정.
    pub fn server_config(&self) -> Result<Arc<rustls::ServerConfig>, NetError> {
        let cert = CertificateDer::from(self.cert_der.clone());
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.clone()));
        let config = rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(|e| NetError::Tls(e.to_string()))?
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .map_err(|e| NetError::Tls(e.to_string()))?;
        Ok(Arc::new(config))
    }
}

/// 서버 인증서 지문을 기록하고, 고정값이 있으면 대조하는 검증기.
#[derive(Debug)]
pub struct PinVerifier {
    pinned: Option<Fingerprint>,
    seen: Mutex<Option<Fingerprint>>,
    provider: Arc<CryptoProvider>,
}

impl PinVerifier {
    /// 핸드셰이크에서 본 서버 지문.
    pub fn seen(&self) -> Option<Fingerprint> {
        self.seen.lock().map(|g| *g).unwrap_or(None)
    }
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fingerprint = Fingerprint::of_der(end_entity.as_ref());
        if let Ok(mut seen) = self.seen.lock() {
            *seen = Some(fingerprint);
        }
        match self.pinned {
            Some(pinned) if pinned != fingerprint => {
                Err(rustls::Error::General("인증서 고정 불일치".to_owned()))
            }
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// 클라이언트 설정과 그 검증기.
pub fn client_config(
    pinned: Option<Fingerprint>,
) -> Result<(Arc<rustls::ClientConfig>, Arc<PinVerifier>), NetError> {
    let verifier = Arc::new(PinVerifier {
        pinned,
        seen: Mutex::new(None),
        provider: provider(),
    });
    let config = rustls::ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| NetError::Tls(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();
    Ok((Arc::new(config), verifier))
}
