use super::*;

#[test]
fn given_generated_identity_when_reloaded_from_der_then_same_fingerprint() {
    let id = ServerIdentity::generate().expect("인증서 생성");
    let reloaded = ServerIdentity::from_der(id.cert_der().to_vec(), id.key_der().to_vec());
    assert_eq!(reloaded.fingerprint(), id.fingerprint());
}

#[test]
fn given_identity_when_debug_printed_then_key_not_shown() {
    let id = ServerIdentity::generate().expect("인증서 생성");
    assert_eq!(
        format!("{id:?}"),
        format!("ServerIdentity({})", id.fingerprint().short())
    );
}

#[test]
fn given_identity_when_server_config_built_then_config_is_returned() {
    let id = ServerIdentity::generate().expect("인증서 생성");
    assert!(id.server_config().is_ok());
}

#[test]
fn given_garbage_key_when_server_config_built_then_tls_error() {
    let id = ServerIdentity::from_der(vec![1, 2, 3], vec![4, 5, 6]);
    assert!(matches!(id.server_config(), Err(NetError::Tls(_))));
}
