use super::*;

const STANDALONE: &str = r#"
mode = "standalone"

[sensor]
id = "3-2-앞"
room = "3-2"
position = "Front"

[detector]
baseline_ms = 60000

[[layout.rooms]]
id = "3-2"
sensors = [{ id = "3-2-앞", position = "Front", inside_boundary_dbm = -75 }]
"#;

#[test]
fn given_standalone_config_when_parsed_then_mode_sensor_and_detector_overrides_kept() {
    let config = AppConfig::parse(STANDALONE).expect("올바른 설정");
    assert_eq!(
        (
            config.mode,
            config.detector.baseline_ms,
            config.detector.alert_score
        ),
        (Mode::Standalone, 60_000, 4)
    );
}

#[test]
fn given_standalone_sensor_missing_from_layout_when_parsed_then_error() {
    let text = STANDALONE.replace("id = \"3-2-앞\"\nroom", "id = \"9-9\"\nroom");
    let error = AppConfig::parse(&text).err().map(|e| e.to_string());
    assert_eq!(error, Some("센서 9-9가 [layout]에 없다".to_owned()));
}

#[test]
fn given_layout_sensor_with_place_when_parsed_then_place_kept_and_absent_place_is_none() {
    let text = STANDALONE.replace(
        "position = \"Front\", inside_boundary_dbm",
        "position = \"Front\", place = \"맨 뒤 동쪽\", inside_boundary_dbm",
    );
    let place = |text: &str| {
        AppConfig::parse(text).expect("올바른 설정").layout.rooms[0].sensors[0]
            .place
            .clone()
    };
    assert_eq!(
        [place(&text), place(STANDALONE)],
        [Some("맨 뒤 동쪽".to_owned()), None]
    );
}

fn missing_section(result: &Result<AppConfig, ConfigError>) -> Option<&'static str> {
    match result {
        Err(ConfigError::Missing { section, .. }) => Some(section),
        _ => None,
    }
}

#[test]
fn given_modes_without_required_sections_when_parsed_then_missing_section_errors() {
    let results = [
        AppConfig::parse("mode = \"standalone\""),
        AppConfig::parse(
            "mode = \"sensor\"\n[sensor]\nid = \"a\"\nroom = \"r\"\nposition = \"Back\"",
        ),
        AppConfig::parse("mode = \"server\""),
        AppConfig::parse("mode = \"viewer\""),
    ];
    let sections: Vec<Option<&str>> = results.iter().map(missing_section).collect();
    assert_eq!(
        sections,
        vec![
            Some("sensor"),
            Some("remote"),
            Some("server"),
            Some("remote")
        ]
    );
}

#[test]
fn given_server_and_viewer_configs_when_parsed_then_building_default_and_address_optional() {
    let server = AppConfig::parse(
        "mode = \"server\"\n[server]\nbind = \"0.0.0.0:47100\"\ndashboard_bind = \"0.0.0.0:47180\"",
    );
    let viewer = AppConfig::parse("mode = \"viewer\"\n[remote]\ncode = \"12345678\"");
    assert_eq!(
        (
            server.ok().map(|c| c.server.map(|s| s.building)),
            viewer.ok().map(|c| c.remote.map(|r| r.address))
        ),
        (Some(Some("시험본부".to_owned())), Some(Some(None)))
    );
}

#[test]
fn given_bad_toml_or_duplicate_rooms_when_parsed_then_errors() {
    let dup = "mode = \"viewer\"\n[remote]\ncode = \"1\"\n[[layout.rooms]]\nid = \"a\"\nsensors = []\n[[layout.rooms]]\nid = \"a\"\nsensors = []";
    assert_eq!(
        (
            matches!(AppConfig::parse("mode = "), Err(ConfigError::Toml(_))),
            matches!(AppConfig::parse(dup), Err(ConfigError::Layout(_)))
        ),
        (true, true)
    );
}

#[test]
fn given_sensor_section_when_info_built_then_platform_attached() {
    let config = AppConfig::parse(STANDALONE).expect("올바른 설정");
    let info = config.sensor_info(Platform::MacOs).expect("센서 정보");
    assert_eq!(
        (info.id, info.platform),
        (SensorId::new("3-2-앞"), Platform::MacOs)
    );
}

#[test]
fn given_modes_when_labelled_then_korean_names() {
    assert_eq!(
        [Mode::Standalone, Mode::Sensor, Mode::Server, Mode::Viewer].map(Mode::label),
        [
            "감독관 화면",
            "교실 센서 (개발자용)",
            "시험본부 서버 (개발자용)",
            "본부 화면 보기 (개발자용)"
        ]
    );
}

#[test]
fn given_args_when_config_path_read_then_flag_value_or_default() {
    let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let default = PathBuf::from("기본.toml");
    assert_eq!(
        (
            config_path_from_args(args(&["--config", "a.toml"]), default.clone()),
            config_path_from_args(args(&["--config"]), default.clone()),
            config_path_from_args(args(&["x"]), default.clone()),
        ),
        (PathBuf::from("a.toml"), default.clone(), default)
    );
}

#[test]
fn given_state_dir_in_config_when_read_then_override_used() {
    let config =
        AppConfig::parse("mode = \"viewer\"\nstate_dir = \"/tmp/상태\"\n[remote]\ncode = \"1\"")
            .expect("설정");
    assert_eq!(config.state_dir(), PathBuf::from("/tmp/상태"));
}

#[test]
fn given_documented_example_configs_when_parsed_then_each_is_valid_for_its_mode() {
    let modes = [
        include_str!("../../../../docs/examples/standalone.toml"),
        include_str!("../../../../docs/examples/server.toml"),
        include_str!("../../../../docs/examples/sensor.toml"),
        include_str!("../../../../docs/examples/viewer.toml"),
    ]
    .map(|text| AppConfig::parse(text).map(|c| c.mode).ok());
    assert_eq!(
        modes,
        [
            Some(Mode::Standalone),
            Some(Mode::Server),
            Some(Mode::Sensor),
            Some(Mode::Viewer)
        ]
    );
}

#[test]
fn given_score_thresholds_out_of_order_when_parsed_then_setting_error() {
    let text = format!("{STANDALONE}\n").replace(
        "baseline_ms = 60000",
        "baseline_ms = 60000\nmedium_score = 99",
    );
    assert!(matches!(
        AppConfig::parse(&text),
        Err(ConfigError::Setting(SettingError::ScoreOrder))
    ));
}

#[test]
fn given_manufacturer_whitelist_without_prefix_when_parsed_then_setting_error() {
    let text = format!(
        "{STANDALONE}\n[[whitelist.entries]]\nManufacturer = {{ company = 76, prefix = [] }}\n"
    );
    assert!(matches!(
        AppConfig::parse(&text),
        Err(ConfigError::Setting(SettingError::EmptyManufacturerPrefix(
            76
        )))
    ));
}

#[test]
fn given_remote_fingerprint_when_parsed_then_valid_one_kept_and_bad_one_rejected() {
    let good = AppConfig::parse(&format!(
        "mode = \"viewer\"\n[remote]\ncode = \"1\"\nfingerprint = \"{}\"",
        "ab".repeat(32)
    ))
    .map(|c| c.expected_fingerprint().flatten().map(|f| f.to_string()));
    let bad = AppConfig::parse("mode = \"viewer\"\n[remote]\ncode = \"1\"\nfingerprint = \"짧음\"");
    assert_eq!(
        (good.ok(), matches!(bad, Err(ConfigError::BadFingerprint))),
        (Some(Some("ab".repeat(32))), true)
    );
}

#[test]
fn given_server_binds_when_checked_then_unspecified_and_public_addresses_warned() {
    let text = |bind: &str, dash: &str| {
        format!("mode = \"server\"\n[server]\nbind = \"{bind}\"\ndashboard_bind = \"{dash}\"")
    };
    let count = |bind: &str, dash: &str| {
        AppConfig::parse(&text(bind, dash))
            .map(|c| c.bind_warnings().len())
            .ok()
    };
    assert_eq!(
        [
            count("10.0.0.5:1", "192.168.0.5:2"),
            count("0.0.0.0:1", "127.0.0.1:2"),
            count("[::]:1", "203.0.113.5:2"),
            count("[fd00::1]:1", "[fe80::1]:2")
        ],
        [Some(0), Some(1), Some(2), Some(0)]
    );
}
