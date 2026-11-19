//! S006 one-shot validation of the native account flow against the test issuer.

#[cfg(test)]
mod tests;
mod tokens;

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use frust_auth_session::{AuthSession, AuthSessionOutcome, AuthSessionRequest};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use url::{Url, form_urlencoded};

const ISSUER: &str = "https://account-test.kodma.app";
const CLIENT_ID: &str = "kodma-canvas-macos-test";
const RESOURCE: &str = "https://canvas-api-test.kodma.app";
const CALLBACK_SCHEME: &str = "app.kodma.canvas";
const REDIRECT_URI: &str = "app.kodma.canvas:/oauth/callback";
const SCOPES: &str = "openid offline_access kodma:access";
const BROWSER_TIMEOUT: Duration = Duration::from_secs(600);
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Discovery,
    Random,
    Browser,
    Cancelled,
    Timeout,
    CallbackUri,
    CallbackState,
    CallbackIssuer,
    CallbackResponse,
    TokenExchange,
    Keys,
    IdToken,
    AccessToken,
    SubjectMismatch,
    Clock,
}

pub fn start_validation() {
    if thread::Builder::new()
        .name("kodma-account-validation".into())
        .spawn(|| match validate_login() {
            Ok(()) => eprintln!("Kodma Canvas account-test: identity-verified"),
            Err(error) => eprintln!("Kodma Canvas account-test: {error:?}"),
        })
        .is_err()
    {
        eprintln!("Kodma Canvas account-test: worker-start-failed");
    }
}

fn validate_login() -> Result<(), Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder().https_only(true).max_redirects(0).timeout_global(Some(HTTP_TIMEOUT)).build().into();
    let discovery = Discovery::fetch(&agent)?;
    let attempt = Attempt::new()?;
    let request =
        AuthSessionRequest { url: attempt.authorization_url(discovery.authorization), callback_scheme: CALLBACK_SCHEME.into(), ephemeral: false };
    eprintln!("Kodma Canvas account-test: browser-requested");
    let response = wait_for_browser(AuthSession::start(request)).ok_or(Error::Timeout)?.map_err(|_| Error::Browser)?;
    let callback = match response {
        AuthSessionOutcome::Callback(callback) => callback,
        AuthSessionOutcome::Cancelled => return Err(Error::Cancelled),
        _ => return Err(Error::Browser),
    };
    let grant = attempt.accept(&callback)?;
    eprintln!("Kodma Canvas account-test: callback-validated");
    let tokens = grant.exchange(&agent, &discovery.token)?;
    eprintln!("Kodma Canvas account-test: token-exchanged");
    let keys = fetch_json(&agent, discovery.jwks.as_str()).ok_or(Error::Keys)?;
    tokens::verify(&tokens, &keys, &grant.nonce)
}

#[derive(Deserialize)]
struct Metadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    revocation_endpoint: String,
    code_challenge_methods_supported: Vec<String>,
}

struct Discovery {
    authorization: Url,
    token: Url,
    jwks: Url,
}

impl Discovery {
    fn fetch(agent: &ureq::Agent) -> Result<Self, Error> {
        let discovery_url = format!("{ISSUER}/.well-known/openid-configuration");
        let metadata = fetch_json(agent, &discovery_url).ok_or(Error::Discovery)?;
        Self::from_metadata(metadata)
    }

    fn from_metadata(metadata: Metadata) -> Result<Self, Error> {
        if metadata.issuer != ISSUER || !metadata.code_challenge_methods_supported.iter().any(|method| method == "S256") {
            return Err(Error::Discovery);
        }
        endpoint(&metadata.revocation_endpoint)?;
        Ok(Self {
            authorization: endpoint(&metadata.authorization_endpoint)?,
            token: endpoint(&metadata.token_endpoint)?,
            jwks: endpoint(&metadata.jwks_uri)?,
        })
    }
}

fn endpoint(value: &str) -> Result<Url, Error> {
    let url = Url::parse(value).map_err(|_| Error::Discovery)?;
    if url.origin().ascii_serialization() != ISSUER || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(Error::Discovery);
    }
    Ok(url)
}

fn fetch_json<T: DeserializeOwned>(agent: &ureq::Agent, url: &str) -> Option<T> {
    response_json(agent.get(url).call().ok()?)
}

fn response_json<T: DeserializeOwned>(mut response: ureq::http::Response<ureq::Body>) -> Option<T> {
    if !response.status().is_success() {
        return None;
    }
    let body = response.body_mut().with_config().limit(128 * 1024).read_to_vec().ok()?;
    serde_json::from_slice(&body).ok()
}

struct Attempt {
    state: String,
    nonce: String,
    verifier: String,
}

impl Attempt {
    fn new() -> Result<Self, Error> {
        let random = SystemRandom::new();
        let secret = || {
            let mut bytes = [0u8; 32];
            random.fill(&mut bytes).map_err(|_| Error::Random)?;
            Ok(URL_SAFE_NO_PAD.encode(bytes))
        };
        Ok(Self { state: secret()?, nonce: secret()?, verifier: secret()? })
    }

    fn authorization_url(&self, mut endpoint: Url) -> String {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(&self.verifier));
        endpoint.query_pairs_mut().clear().extend_pairs([
            ("response_type", "code"),
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
            ("scope", SCOPES),
            ("resource", RESOURCE),
            ("state", &self.state),
            ("nonce", &self.nonce),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ]);
        endpoint.into()
    }

    /// Consumes this attempt before any exchange. A late callback from an older attempt
    /// cannot satisfy the fresh state, including when the system presents the same scheme.
    fn accept(self, callback: &str) -> Result<Grant, Error> {
        let (uri, query) = callback.split_once('?').ok_or(Error::CallbackUri)?;
        if uri != REDIRECT_URI || callback.len() > 8192 || query.contains('#') {
            return Err(Error::CallbackUri);
        }
        let mut fields = CallbackFields::default();
        for (name, value) in form_urlencoded::parse(query.as_bytes()) {
            let field = match name.as_ref() {
                "state" => &mut fields.state,
                "iss" => &mut fields.issuer,
                "code" => &mut fields.code,
                "error" => &mut fields.error,
                _ => continue,
            };
            if field.replace(value).is_some() {
                return Err(Error::CallbackResponse);
            }
        }
        if fields.state.as_deref() != Some(&self.state) {
            return Err(Error::CallbackState);
        }
        if fields.issuer.as_deref() != Some(ISSUER) {
            return Err(Error::CallbackIssuer);
        }
        if fields.error.is_some() {
            return Err(Error::CallbackResponse);
        }
        let code = fields.code.filter(|code| !code.is_empty()).ok_or(Error::CallbackResponse)?.into_owned();
        Ok(Grant { code, nonce: self.nonce, verifier: self.verifier })
    }
}

#[derive(Default)]
struct CallbackFields<'a> {
    state: Option<std::borrow::Cow<'a, str>>,
    issuer: Option<std::borrow::Cow<'a, str>>,
    code: Option<std::borrow::Cow<'a, str>>,
    error: Option<std::borrow::Cow<'a, str>>,
}

struct Grant {
    code: String,
    nonce: String,
    verifier: String,
}

impl Grant {
    fn exchange(&self, agent: &ureq::Agent, endpoint: &Url) -> Result<tokens::TokenResponse, Error> {
        // This public client uses PKCE. Discovery doesn't currently advertise "none".
        let response = agent
            .post(endpoint.as_str())
            .send_form([
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT_ID),
                ("code", &self.code),
                ("redirect_uri", REDIRECT_URI),
                ("code_verifier", &self.verifier),
            ])
            .map_err(|_| Error::TokenExchange)?;
        response_json(response).ok_or(Error::TokenExchange)
    }
}

struct WorkerWake(thread::Thread);

impl Wake for WorkerWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn wait_for_browser<F: Future>(future: F) -> Option<F::Output> {
    let deadline = Instant::now() + BROWSER_TIMEOUT;
    let waker = Waker::from(Arc::new(WorkerWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(outcome) => return Some(outcome),
            Poll::Pending => thread::park_timeout(remaining),
        }
    }
    None
}
