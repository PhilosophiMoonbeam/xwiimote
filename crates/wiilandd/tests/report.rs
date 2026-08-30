use wiilandd::report::{ArgError, parse_args};

#[test]
fn help_is_not_forwarded_when_first_argument_is_help() {
    let args = vec!["--help".to_owned()];
    // The binary handles this before parsing; the scanner's representation retains
    // the argument so callers can make the same decision without losing argv data.
    let parsed = parse_args(&args).expect("help should parse");
    assert_eq!(parsed.device, None);
    assert_eq!(parsed.extra, vec!["--help"]);
}

#[test]
fn scanner_matches_conflict_contract() {
    let args = vec!["4".to_owned(), "--profile".to_owned(), "both".to_owned()];
    assert_eq!(
        parse_args(&args),
        Err(ArgError::Conflicting("--profile".to_owned()))
    );

    let args = vec![
        "4".to_owned(),
        "--trace-events=ir".to_owned(),
        "--trace-events=axes".to_owned(),
    ];
    assert_eq!(parse_args(&args), Err(ArgError::MultipleTraceSelectors));
}

#[test]
fn value_options_are_forwarded_without_scanning_their_value() {
    let args = vec![
        "4".to_owned(),
        "--config".to_owned(),
        "--device-name-that-is-not-a-report-option".to_owned(),
        "--trace-events=motion-plus".to_owned(),
    ];
    let parsed = parse_args(&args).expect("valid trace arguments");
    assert_eq!(
        parsed.extra[0..2],
        ["--config", "--device-name-that-is-not-a-report-option"]
    );
    assert_eq!(parsed.trace_selectors, 1);
}
