use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

use super::{CLIENT_ID, Error, ISSUER, RESOURCE};

#[derive(Deserialize)]
pub(super) struct TokenResponse {
    id_token: Option<String>,
    access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct Identity {
    pub sub: String,
    pub email: Option<String>,
    pub expires_at: u64,
    login_nonce: String,
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
    email: Option<String>,
    email_verified: Option<bool>,
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

pub(super) fn now() -> Result<u64, Error> {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).map_err(|_| Error::Clock)
}

pub(super) fn verify(tokens: &TokenResponse, keys: &KeySet, nonce: &str) -> Result<Identity, Error> {
    verify_at(tokens, keys, nonce, now()?)
}

fn verify_at(tokens: &TokenResponse, keys: &KeySet, nonce: &str, now: u64) -> Result<Identity, Error> {
    let id = identity_claims(tokens.id_token.as_deref().ok_or(Error::IdToken)?, keys, now)?;
    if id.nonce.as_deref() != Some(nonce) {
        return Err(Error::IdToken);
    }
    let access = access_claims(&tokens.access_token, keys, now)?;
    if access.sub != id.sub {
        return Err(Error::SubjectMismatch);
    }
    Ok(Identity { sub: id.sub, email: id.email.filter(|_| id.email_verified == Some(true)), expires_at: access.exp, login_nonce: nonce.into() })
}

pub(super) fn verify_refresh(tokens: &TokenResponse, keys: &KeySet, identity: &Identity) -> Result<u64, Error> {
    verify_refresh_at(tokens, keys, identity, now()?)
}

fn verify_refresh_at(tokens: &TokenResponse, keys: &KeySet, identity: &Identity, now: u64) -> Result<u64, Error> {
    let access = access_claims(&tokens.access_token, keys, now)?;
    if access.sub != identity.sub {
        return Err(Error::SubjectMismatch);
    }
    if let Some(token) = &tokens.id_token {
        let id = identity_claims(token, keys, now)?;
        if id.sub != identity.sub {
            return Err(Error::SubjectMismatch);
        }
        if id.nonce.as_deref().is_some_and(|nonce| nonce != identity.login_nonce) {
            return Err(Error::IdToken);
        }
    }
    Ok(access.exp)
}

fn identity_claims(token: &str, keys: &KeySet, now: u64) -> Result<Claims, Error> {
    let (id_header, id) = claims(token, keys, now).ok_or(Error::IdToken)?;
    if !matches!(id_header.typ.as_deref(), Some("JWT") | None) || !matches!(id.aud.values(), [aud] if aud == CLIENT_ID) {
        return Err(Error::IdToken);
    }
    Ok(id)
}

fn access_claims(token: &str, keys: &KeySet, now: u64) -> Result<Claims, Error> {
    let (access_header, access) = claims(token, keys, now).ok_or(Error::AccessToken)?;
    if access_header.typ.as_deref() != Some("at+jwt")
        || !access.aud.values().iter().any(|aud| aud == RESOURCE)
        || !access.scope.as_deref().is_some_and(|scope| scope.split_ascii_whitespace().any(|s| s == "kodma:access"))
    {
        return Err(Error::AccessToken);
    }
    Ok(access)
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
