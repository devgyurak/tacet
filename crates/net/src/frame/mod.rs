//! 줄 단위 JSON 프레임. 메시지 하나가 한 줄이다.
//!
//! 서버는 센서 보고서를 신뢰하지 않는다(AGENTS.md §2 규칙 10). 너무 긴 줄은 연결을 끊고, 형식이 틀린
//! 줄은 버린다.

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::NetError;

#[cfg(test)]
mod tests;

/// 한 줄의 최대 바이트.
pub const MAX_FRAME: usize = 256 * 1024;

/// 메시지 하나를 한 줄로 쓴다.
pub async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    msg: &T,
) -> Result<(), NetError> {
    let mut line = serde_json::to_vec(msg).map_err(|e| NetError::Malformed(e.to_string()))?;
    line.push(b'\n');
    writer.write_all(&line).await?;
    writer.flush().await?;
    Ok(())
}

/// 한 줄을 읽어 메시지로 푼다. 연결이 끝났으면 `Ok(None)`.
pub async fn read_frame<R: AsyncBufRead + Unpin, T: DeserializeOwned>(
    reader: &mut R,
) -> Result<Option<T>, NetError> {
    let mut line = Vec::new();
    let limit = u64::try_from(MAX_FRAME).unwrap_or(u64::MAX) + 1;
    let read = (&mut *reader)
        .take(limit)
        .read_until(b'\n', &mut line)
        .await?;
    if read == 0 {
        return Ok(None);
    }
    if line.last() != Some(&b'\n') {
        return Err(if line.len() > MAX_FRAME {
            NetError::FrameTooLong
        } else {
            NetError::Closed
        });
    }
    serde_json::from_slice(&line)
        .map(Some)
        .map_err(|e| NetError::Malformed(e.to_string()))
}
