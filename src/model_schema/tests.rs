use super::validate_ts_optional_flag;

#[test]
fn ts_optional_ok_on_option_field() {
    validate_ts_optional_flag(true, true).unwrap();
}

#[test]
fn ts_optional_ok_when_flag_unset() {
    // Flag not set: valid regardless of optionality.
    validate_ts_optional_flag(true, false).unwrap();
    validate_ts_optional_flag(false, false).unwrap();
}

#[test]
fn ts_optional_err_on_non_option_field() {
    let err = validate_ts_optional_flag(false, true).unwrap_err();
    assert!(err.contains("ts_optional"));
    assert!(err.contains("Option<T>"));
}
