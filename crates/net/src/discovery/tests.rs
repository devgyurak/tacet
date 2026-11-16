use super::*;

#[test]
fn given_advertised_server_when_browsing_briefly_then_browse_completes_with_deduplicated_list() {
    let fp = Fingerprint::of_der(b"synthetic");
    let advertiser = advertise("시험본부-시험", 45_678, &fp);
    let found = discover(Duration::from_millis(300));
    let deduped = found
        .as_ref()
        .map(|list| list.windows(2).all(|w| w[0] != w[1]))
        .unwrap_or(true);
    assert_eq!(
        (advertiser.is_ok(), found.is_ok(), deduped),
        (true, true, true)
    );
}
