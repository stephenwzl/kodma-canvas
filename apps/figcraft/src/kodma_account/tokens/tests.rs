use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
    rand::SystemRandom,
    signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair},
};
use serde_json::{Value, json};

use super::{CLIENT_ID, Error, ISSUER, KeySet, RESOURCE, TokenResponse, verify_at};

const NOW: u64 = 100;
const NONCE: &str = "synthetic-login-nonce";

fn id_claims() -> Value {
    json!({"iss": ISSUER, "aud": CLIENT_ID, "sub": "test-user", "exp": NOW + 300, "nonce": NONCE})
}

fn access_claims() -> Value {
    json!({"iss": ISSUER, "aud": [RESOURCE], "sub": "test-user", "exp": NOW + 300, "scope": "openid kodma:access"})
}

struct Signer {
    random: SystemRandom,
    pair: EcdsaKeyPair,
    keys: KeySet,
}

impl Signer {
    fn new() -> Self {
        let random = SystemRandom::new();
        let algorithm = &ECDSA_P256_SHA256_FIXED_SIGNING;
        let key = EcdsaKeyPair::generate_pkcs8(algorithm, &random).unwrap();
        let pair = EcdsaKeyPair::from_pkcs8(algorithm, key.as_ref(), &random).unwrap();
        let public_key = pair.public_key().as_ref();
        let keys = serde_json::from_value(json!({"keys": [{
            "kid": "current-key", "kty": "EC", "alg": "ES256", "crv": "P-256",
            "x": URL_SAFE_NO_PAD.encode(&public_key[1..33]), "y": URL_SAFE_NO_PAD.encode(&public_key[33..]),
        }]}))
        .unwrap();
        Self { random, pair, keys }
    }

    fn token(&self, claims: Value, typ: &str) -> String {
        let header = json!({"alg": "ES256", "kid": "current-key", "typ": typ});
        let encoded = |value: Value| URL_SAFE_NO_PAD.encode(serde_json::to_vec(&value).unwrap());
        let message = format!("{}.{}", encoded(header), encoded(claims));
        let signature = self.pair.sign(&self.random, message.as_bytes()).unwrap();
        format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature.as_ref()))
    }

    fn check(&self, id: Value, access: Value) -> Result<(), Error> {
        let tokens = TokenResponse { id_token: self.token(id, "JWT"), access_token: self.token(access, "at+jwt") };
        verify_at(&tokens, &self.keys, NONCE, NOW)
    }
}

#[test]
fn verifies_signed_tokens_and_rejects_the_wrong_signing_key() {
    let signer = Signer::new();
    assert_eq!(signer.check(id_claims(), access_claims()), Ok(()));
    let other = Signer::new();
    let tokens = TokenResponse { id_token: other.token(id_claims(), "JWT"), access_token: signer.token(access_claims(), "at+jwt") };
    assert_eq!(verify_at(&tokens, &signer.keys, NONCE, NOW), Err(Error::IdToken));
    let tokens = TokenResponse { id_token: signer.token(id_claims(), "JWT"), access_token: other.token(access_claims(), "at+jwt") };
    assert_eq!(verify_at(&tokens, &signer.keys, NONCE, NOW), Err(Error::AccessToken));
}

#[test]
fn rejects_unbound_identity_and_access_claims_even_with_valid_signatures() {
    let signer = Signer::new();
    for (claim, invalid) in
        [("iss", json!("https://account.kodma.app")), ("aud", json!("wrong-client")), ("exp", json!(NOW)), ("nonce", json!("previous-attempt"))]
    {
        let mut id = id_claims();
        id[claim] = invalid;
        assert_eq!(signer.check(id, access_claims()), Err(Error::IdToken));
    }
    for (claim, invalid, error) in [
        ("aud", json!("https://frame-api-test.kodma.app"), Error::AccessToken),
        ("scope", json!("openid"), Error::AccessToken),
        ("sub", json!("different-user"), Error::SubjectMismatch),
    ] {
        let mut access = access_claims();
        access[claim] = invalid;
        assert_eq!(signer.check(id_claims(), access), Err(error));
    }
}
