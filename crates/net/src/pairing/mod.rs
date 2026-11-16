//! 페어링 코드와 증명.
//!
//! 서버 화면의 10자리 코드를 센서에 입력한다. 센서는 TLS 핸드셰이크에서 받은 서버 인증서 지문과 역할을
//! 코드로 HMAC해 증명을 보낸다. 중간자가 끼면 지문이 달라 증명이 맞지 않는다. 첫 연결 뒤에는 지문을
//! 고정한다(TOFU). 코드는 오프라인 대입에 약하므로 서버는 실패 횟수를 제한한다.

use std::fmt;

use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests;

/// 서버 인증서 지문(DER의 SHA-256).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    /// 인증서 DER로 계산한다.
    pub fn of_der(der: &[u8]) -> Self {
        Self(Sha256::digest(der).into())
    }

    /// 16진수 문자열을 읽는다.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = hex::decode(text.trim()).ok()?;
        Some(Self(bytes.try_into().ok()?))
    }

    /// 사람이 대조하기 쉬운 앞 16자리.
    pub fn short(&self) -> String {
        hex::encode(&self.0[..8])
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// 페어링 코드 글자(헷갈리는 0·O·1·I·L 제외).
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// 페어링 코드 길이(31^10 ≈ 8×10^14). 첫 연결의 중간자가 증명을 얻어도 오프라인 대입이 쉽지 않게 길게 둔다.
pub const CODE_LEN: usize = 10;

/// 새 페어링 코드.
pub fn new_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN)
        .map(|_| char::from(CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())]))
        .collect()
}

/// 사람이 입력한 코드를 비교용으로 정리한다(공백 제거, 대문자).
pub fn normalize(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_uppercase()
}

fn mac(code: &str, fingerprint: &Fingerprint, role: &str) -> Hmac<Sha256> {
    // 입력 실수(소문자, 띄어쓰기, 하이픈)는 같은 코드로 본다.
    let code = normalize(code);
    // HMAC은 어떤 길이의 키든 받으므로 new_from_slice는 실패하지 않는다.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(code.as_bytes())
        .unwrap_or_else(|_| unreachable!("HMAC 키 길이 제한 없음"));
    mac.update(&fingerprint.0);
    mac.update(role.as_bytes());
    mac
}

/// 증명(16진수).
pub fn proof(code: &str, fingerprint: &Fingerprint, role: &str) -> String {
    hex::encode(mac(code, fingerprint, role).finalize().into_bytes())
}

/// 증명을 상수 시간으로 확인한다.
pub fn verify(code: &str, fingerprint: &Fingerprint, role: &str, proof: &str) -> bool {
    hex::decode(proof).is_ok_and(|bytes| mac(code, fingerprint, role).verify_slice(&bytes).is_ok())
}
