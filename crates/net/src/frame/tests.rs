use super::*;
use tokio::io::BufReader;

#[tokio::test]
async fn given_message_when_written_and_read_then_round_trips() {
    let mut buf = Vec::new();
    write_frame(&mut buf, &vec![1u32, 2, 3])
        .await
        .expect("버퍼 쓰기는 실패하지 않는다");
    let read: Option<Vec<u32>> = read_frame(&mut BufReader::new(buf.as_slice()))
        .await
        .expect("유효한 프레임");
    assert_eq!(read, Some(vec![1, 2, 3]));
}

#[tokio::test]
async fn given_empty_stream_when_read_then_none() {
    let read: Option<u32> = read_frame(&mut BufReader::new(&b""[..]))
        .await
        .expect("빈 스트림은 끝이다");
    assert_eq!(read, None);
}

#[tokio::test]
async fn given_malformed_line_when_read_then_malformed_error() {
    let read: Result<Option<u32>, NetError> =
        read_frame(&mut BufReader::new(&b"{not json\n"[..])).await;
    assert!(matches!(read, Err(NetError::Malformed(_))));
}

#[tokio::test]
async fn given_line_longer_than_limit_when_read_then_frame_too_long() {
    let long = vec![b'1'; MAX_FRAME + 10];
    let read: Result<Option<u32>, NetError> =
        read_frame(&mut BufReader::new(long.as_slice())).await;
    assert!(matches!(read, Err(NetError::FrameTooLong)));
}

#[tokio::test]
async fn given_truncated_line_without_newline_when_read_then_closed() {
    let read: Result<Option<u32>, NetError> = read_frame(&mut BufReader::new(&b"12"[..])).await;
    assert!(matches!(read, Err(NetError::Closed)));
}
