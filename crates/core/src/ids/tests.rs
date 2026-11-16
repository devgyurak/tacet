use super::*;

#[test]
fn given_doc_random_mac_when_bits_checked_then_locally_administered_and_unicast() {
    let mac = Mac([0x02, 0x00, 0x5E, 0x00, 0x53, 0x01]);
    assert_eq!(
        (mac.is_locally_administered(), mac.is_multicast()),
        (true, false)
    );
}

#[test]
fn given_doc_mac_when_bits_checked_then_globally_administered() {
    let mac = Mac([0x00, 0x00, 0x5E, 0x00, 0x53, 0x01]);
    assert!(!mac.is_locally_administered());
}

#[test]
fn given_colon_and_dash_text_when_parsed_then_same_mac() {
    let colon = Mac::parse("00:00:5e:00:53:2A");
    let dash = Mac::parse("00-00-5E-00-53-2a");
    assert_eq!(
        (colon, dash),
        (
            Some(Mac([0, 0, 0x5E, 0, 0x53, 0x2A])),
            Some(Mac([0, 0, 0x5E, 0, 0x53, 0x2A]))
        )
    );
}

#[test]
fn given_malformed_texts_when_parsed_then_none() {
    let parsed = [
        Mac::parse("00:00:5E:00:53"),
        Mac::parse("00:00:5E:00:53:2AA"),
        Mac::parse("zz:00:5E:00:53:01"),
    ];
    assert_eq!(parsed, [None, None, None]);
}

#[test]
fn given_mac_when_displayed_then_uppercase_colon_form() {
    assert_eq!(
        Mac([0, 0, 0x5E, 0, 0x53, 0x2A]).to_string(),
        "00:00:5E:00:53:2A"
    );
}

#[test]
fn given_short_uuid_when_widened_then_round_trips_to_short() {
    assert_eq!(ServiceUuid::from_u16(0xFE2C).as_u16(), Some(0xFE2C));
}

#[test]
fn given_custom_128_bit_uuid_when_shortened_then_none() {
    assert_eq!(ServiceUuid(0x1234).as_u16(), None);
}

#[test]
fn given_ids_when_displayed_then_text_is_kept() {
    assert_eq!(
        (
            SensorId::new("3-2-앞").to_string(),
            RoomId::new("3-2").to_string()
        ),
        ("3-2-앞".to_owned(), "3-2".to_owned())
    );
}
