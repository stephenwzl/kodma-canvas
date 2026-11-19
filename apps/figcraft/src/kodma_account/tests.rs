use super::{Attempt, CLIENT_ID, Discovery, Error, ISSUER, LoginGate, Metadata, REDIRECT_URI, RESOURCE, SCOPES, endpoint, wait_for_browser};
use std::time::Duration;
use url::{Url, form_urlencoded};

fn callback(uri: &str, state: &str, issuer: &str) -> String {
    let query = form_urlencoded::Serializer::new(String::new()).extend_pairs([("code", "test+code"), ("state", state), ("iss", issuer)]).finish();
    format!("{uri}?{query}")
}

#[test]
fn accepts_only_the_exact_uri_issuer_and_current_attempt() {
    let cases = [
        ("app.kodma.canvas://oauth/callback", None, ISSUER, Error::CallbackUri),
        ("app.kodma.canvas:/other", None, ISSUER, Error::CallbackUri),
        (REDIRECT_URI, None, "https://account.kodma.app", Error::CallbackIssuer),
        (REDIRECT_URI, Some("wrong-state"), ISSUER, Error::CallbackState),
    ];
    for (uri, state, issuer, expected) in cases {
        let attempt = Attempt::new().unwrap();
        let later = callback(uri, state.unwrap_or(&attempt.state), issuer);
        assert_eq!(attempt.accept(&later).err(), Some(expected));
    }

    let previous = Attempt::new().unwrap();
    let current = Attempt::new().unwrap();
    assert_ne!(current.nonce, previous.nonce);
    assert_ne!(current.verifier, previous.verifier);
    let old_callback = callback(REDIRECT_URI, &previous.state, ISSUER);
    assert_eq!(current.accept(&old_callback).err(), Some(Error::CallbackState));
    assert_eq!(previous.accept(&old_callback).unwrap().code, "test+code");
}

#[test]
fn rejects_ambiguous_failed_and_missing_grants() {
    for suffix in ["&code=another", "&error=access_denied", "&state=another", "&iss=another", "#fragment"] {
        let attempt = Attempt::new().unwrap();
        let valid = callback(REDIRECT_URI, &attempt.state, ISSUER);
        assert!(attempt.accept(&format!("{valid}{suffix}")).is_err());
    }
    let attempt = Attempt::new().unwrap();
    let no_code = callback(REDIRECT_URI, &attempt.state, ISSUER).replace("code=test%2Bcode", "code=");
    assert_eq!(attempt.accept(&no_code).err(), Some(Error::CallbackResponse));
}

#[test]
fn builds_the_registered_pkce_request() {
    let attempt = Attempt {
        state: "test-state".into(),
        nonce: "test-nonce".into(),
        // RFC 7636 Appendix B.
        verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into(),
    };
    let endpoint = endpoint(&format!("{ISSUER}/authorize")).unwrap();
    let url = Url::parse(&attempt.authorization_url(endpoint)).unwrap();
    let params: Vec<_> = url.query_pairs().into_owned().collect();
    assert_eq!(
        params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>(),
        [
            ("response_type", "code"),
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("scope", SCOPES),
            ("resource", RESOURCE),
            ("state", "test-state"),
            ("nonce", "test-nonce"),
            ("code_challenge", "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"),
            ("code_challenge_method", "S256"),
        ]
    );
}

fn metadata() -> Metadata {
    Metadata {
        issuer: ISSUER.into(),
        authorization_endpoint: format!("{ISSUER}/authorize"),
        token_endpoint: format!("{ISSUER}/token"),
        jwks_uri: format!("{ISSUER}/jwks"),
        revocation_endpoint: format!("{ISSUER}/revoke"),
        code_challenge_methods_supported: vec!["S256".into()],
    }
}

#[test]
fn discovery_pins_the_issuer_and_endpoint_origin() {
    assert!(Discovery::from_metadata(metadata()).is_ok());
    let mut bad = metadata();
    bad.issuer = "https://account.kodma.app".into();
    assert_eq!(Discovery::from_metadata(bad).err(), Some(Error::Discovery));
    for token in [
        "http://account-test.kodma.app/token",
        "https://other.kodma.app/token",
        "https://account-test.kodma.app:444/token",
        "https://user@account-test.kodma.app/token",
    ] {
        let bad = Metadata { token_endpoint: token.into(), ..metadata() };
        assert_eq!(Discovery::from_metadata(bad).err(), Some(Error::Discovery));
    }
}

#[test]
fn cancelled_browser_results_cannot_be_committed() {
    let gate = LoginGate::default();
    assert!(gate.cancel());
    assert_eq!(wait_for_browser(std::future::ready("late-callback"), Some(&gate), Duration::from_secs(1)), Err(Error::Cancelled));
    assert!(!gate.begin_commit());

    let gate = LoginGate::default();
    assert!(gate.begin_commit());
    assert!(!gate.cancel());
    assert!(!gate.cancelled());
}

#[test]
fn a_browser_that_never_returns_times_out() {
    assert_eq!(wait_for_browser(std::future::pending::<()>(), None, Duration::from_millis(1)), Err(Error::Timeout));
}
