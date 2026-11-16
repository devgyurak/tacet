use super::*;

fn fp() -> Fingerprint {
    Fingerprint::of_der(b"synthetic certificate")
}

#[test]
fn given_new_codes_when_generated_then_ten_unambiguous_characters() {
    let code = new_code();
    assert_eq!(
        (code.len(), code.bytes().all(|b| CODE_ALPHABET.contains(&b))),
        (CODE_LEN, true)
    );
}

#[test]
fn given_code_typed_with_lowercase_spaces_and_hyphens_when_verified_then_same_code() {
    assert!(verify(
        "ab23-cd 45ef",
        &fp(),
        "viewer",
        &proof("AB23CD45EF", &fp(), "viewer")
    ));
}

#[test]
fn given_matching_code_fingerprint_and_role_when_verified_then_true() {
    assert!(verify(
        "12345678",
        &fp(),
        "viewer",
        &proof("12345678", &fp(), "viewer")
    ));
}

#[test]
fn given_wrong_code_or_role_or_fingerprint_when_verified_then_false() {
    let p = proof("12345678", &fp(), "viewer");
    let other = Fingerprint::of_der(b"other");
    assert_eq!(
        [
            verify("87654321", &fp(), "viewer", &p),
            verify("12345678", &fp(), "sensor:x", &p),
            verify("12345678", &other, "viewer", &p),
            verify("12345678", &fp(), "viewer", "zz")
        ],
        [false, false, false, false]
    );
}

#[test]
fn given_fingerprint_when_printed_and_parsed_then_round_trips_and_short_is_16_hex() {
    let f = fp();
    assert_eq!(
        (Fingerprint::parse(&f.to_string()), f.short().len()),
        (Some(f), 16)
    );
}

#[test]
fn given_bad_fingerprint_text_when_parsed_then_none() {
    assert_eq!(
        (Fingerprint::parse("abc"), Fingerprint::parse("00")),
        (None, None)
    );
}
