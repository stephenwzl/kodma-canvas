use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};
use serde::Deserialize;

use super::{CLIENT_ID, Error, ISSUER, RESOURCE};

#[derive(Deserialize)]
pub(super) struct TokenResponse {
    id_token: String,
    access_token: String,
}

#[derive(Deserialize)]
pub(super) struct KeySet {
    keys: Vec<Key>,
}

#[derive(Deserialize)]
struct Key {
    kid: String,
    kty: String,
    alg: Option<String>,
    crv: Option<String>,
    x: Option<String>,
    y: Option<String>,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: String,
    typ: Option<String>,
    crit: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Claims {
    iss: String,
    aud: Audience,
    sub: String,
    exp: u64,
    nbf: Option<u64>,
    nonce: Option<String>,
    scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

impl Audience {
    fn values(&self) -> &[String] {
        match self {
            Self::Single(value) => std::slice::from_ref(value),
            Self::Multiple(values) => values,
        }
    }
}

pub(super) fn verify(tokens: &TokenResponse, keys: &KeySet, nonce: &str) -> Result<(), Error> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| Error::Clock)?.as_secs();
    verify_at(tokens, keys, nonce, now)
}

fn verify_at(tokens: &TokenResponse, keys: &KeySet, nonce: &str, now: u64) -> Result<(), Error> {
    let (id_header, id) = claims(&tokens.id_token, keys, now).ok_or(Error::IdToken)?;
    if !matches!(id_header.typ.as_deref(), Some("JWT") | None)
        || !matches!(id.aud.values(), [aud] if aud == CLIENT_ID)
        || id.nonce.as_deref() != Some(nonce)
    {
        return Err(Error::IdToken);
    }
    let (access_header, access) = claims(&tokens.access_token, keys, now).ok_or(Error::AccessToken)?;
    if access_header.typ.as_deref() != Some("at+jwt")
        || !access.aud.values().iter().any(|aud| aud == RESOURCE)
        || !access.scope.as_deref().is_some_and(|scope| scope.split_ascii_whitespace().any(|s| s == "kodma:access"))
    {
        return Err(Error::AccessToken);
    }
    if access.sub != id.sub {
        return Err(Error::SubjectMismatch);
    }
    Ok(())
}

fn claims(token: &str, keys: &KeySet, now: u64) -> Option<(Header, Claims)> {
    let (signed, signature) = token.rsplit_once('.')?;
    let (header, payload) = signed.split_once('.')?;
    let header: Header = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header).ok()?).ok()?;
    if header.alg != "ES256" || header.kid.is_empty() || header.crit.is_some() {
        return None;
    }
    let mut matching = keys.keys.iter().filter(|key| key.kid == header.kid);
    let key = matching.next()?;
    if matching.next().is_some() || key.kty != "EC" || key.crv.as_deref() != Some("P-256") || key.alg.as_deref().is_some_and(|alg| alg != "ES256") {
        return None;
    }
    let x: [u8; 32] = URL_SAFE_NO_PAD.decode(key.x.as_deref()?).ok()?.try_into().ok()?;
    let y: [u8; 32] = URL_SAFE_NO_PAD.decode(key.y.as_deref()?).ok()?.try_into().ok()?;
    // Ring expects an uncompressed SEC1 point: tag 0x04 followed by aligned coordinates.
    let mut point = [4u8; 65];
    point[1..33].copy_from_slice(&x);
    point[33..].copy_from_slice(&y);
    let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, point).verify(signed.as_bytes(), &signature).ok()?;
    let claims: Claims = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    if claims.iss != ISSUER || claims.exp <= now || claims.nbf.is_some_and(|nbf| nbf > now) || claims.sub.trim().is_empty() {
        return None;
    }
    Some((header, claims))
}

#[cfg(test)]
mod tests;
