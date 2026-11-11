//! Java's public-key crypto as converted sources use it (`KeyPairGenerator`, `KeyFactory`,
//! `KeyAgreement`, `Signature`, RSA `Cipher`s): P-256 and RSA over RustCrypto's crates. Keys pass
//! as their DER encodings (a public key's SubjectPublicKeyInfo, a private key's PKCS #8, which is
//! what Java's `getEncoded()` gives), bytes as base64 and big numbers as hexadecimal.

use base64::Engine as _;
use p256::pkcs8::{DecodePrivateKey as _, DecodePublicKey as _, EncodePrivateKey as _};
use rand_core::OsRng;
use rsa::pkcs8::EncodePublicKey as _;
use serde_json::{Value, json};

#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum KeyArgs {
    /// A new key pair: `EC` on P-256 (`secp256r1`), or `RSA` of `bits`.
    Generate {
        algorithm: String,
        #[serde(default)]
        bits: Option<usize>,
    },
    /// What a key holds: an EC public key's point, a private one's scalar, an RSA key's modulus
    /// and exponent.
    Describe { key: String, public: bool },
    /// A P-256 public key from its point (`ECPublicKeySpec(ECPoint(x, y), params)`).
    EcPublic { x: String, y: String },
    /// An RSA public key from its modulus and exponent (`RSAPublicKeySpec(n, e)`).
    RsaPublic { modulus: String, exponent: String },
    /// ECDH: the shared secret's x coordinate (`KeyAgreement.getInstance("ECDH")`).
    Agree { private: String, public: String },
    /// An ECDSA signature over SHA-256, DER-encoded (`SHA256withECDSA`).
    Sign { private: String, data: String },
    Verify {
        public: String,
        data: String,
        signature: String,
    },
    /// RSA with PKCS #1 v1.5 padding, or OAEP over `hash` (its mask over `mgf_hash`).
    #[serde(rename_all = "camelCase")]
    Rsa {
        key: String,
        encrypt: bool,
        #[serde(default)]
        oaep: bool,
        #[serde(default)]
        hash: String,
        #[serde(default)]
        mgf_hash: String,
        data: String,
    },
}

fn bytes(text: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|error| format!("not base64: {error}"))
}

fn encode(data: impl AsRef<[u8]>) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn hex_bytes(text: &str, width: usize) -> Result<Vec<u8>, String> {
    let text = text.trim_start_matches('-');
    let padded = if text.len() % 2 == 1 {
        format!("0{text}")
    } else {
        text.to_owned()
    };
    let mut out = (0..padded.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&padded[at..at + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|error| format!("not hexadecimal: {error}"))?;
    while out.len() > width && out.first() == Some(&0) {
        out.remove(0);
    }
    if out.len() > width {
        return Err("a coordinate is too long for P-256".into());
    }
    while out.len() < width {
        out.insert(0, 0);
    }
    Ok(out)
}

fn hex(data: &[u8]) -> String {
    let text: String = data.iter().map(|byte| format!("{byte:02x}")).collect();
    let trimmed = text.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn ec_secret(private: &str) -> Result<p256::SecretKey, String> {
    let der = bytes(private)?;
    p256::SecretKey::from_pkcs8_der(&der)
        .or_else(|_| p256::SecretKey::from_sec1_der(&der))
        .map_err(|error| format!("not a P-256 private key: {error}"))
}

fn ec_public(public: &str) -> Result<p256::PublicKey, String> {
    let der = bytes(public)?;
    p256::PublicKey::from_public_key_der(&der)
        .or_else(|_| p256::PublicKey::from_sec1_bytes(&der))
        .map_err(|error| format!("not a P-256 public key: {error}"))
}

fn ec_public_der(public: &p256::PublicKey) -> Result<String, String> {
    use p256::pkcs8::EncodePublicKey as _;
    public
        .to_public_key_der()
        .map(|document| encode(document.as_bytes()))
        .map_err(|error| error.to_string())
}

pub(crate) fn run(args: &str) -> Result<String, String> {
    let args: KeyArgs =
        serde_json::from_str(args).map_err(|error| format!("invalid arguments: {error}"))?;
    let answer: Value = match args {
        KeyArgs::Generate { algorithm, bits } => match algorithm.to_ascii_uppercase().as_str() {
            "EC" | "ECDH" | "ECDSA" => {
                let secret = p256::SecretKey::random(&mut OsRng);
                let private = secret.to_pkcs8_der().map_err(|error| error.to_string())?;
                json!({
                    "public": ec_public_der(&secret.public_key())?,
                    "private": encode(private.as_bytes()),
                })
            }
            "RSA" => {
                let private = rsa::RsaPrivateKey::new(&mut OsRng, bits.unwrap_or(2048))
                    .map_err(|error| error.to_string())?;
                let public = rsa::RsaPublicKey::from(&private);
                json!({
                    "public": encode(public.to_public_key_der().map_err(|error| error.to_string())?.as_bytes()),
                    "private": encode(private.to_pkcs8_der().map_err(|error| error.to_string())?.as_bytes()),
                })
            }
            other => return Err(format!("no {other} key pairs")),
        },
        KeyArgs::Describe { key, public } => {
            use rsa::traits::PublicKeyParts as _;
            if public {
                if let Ok(point) = ec_public(&key) {
                    let encoded =
                        p256::elliptic_curve::sec1::ToEncodedPoint::to_encoded_point(&point, false);
                    json!({
                        "kind": "EC",
                        "x": hex(encoded.x().map(|x| x.as_slice()).unwrap_or_default()),
                        "y": hex(encoded.y().map(|y| y.as_slice()).unwrap_or_default()),
                    })
                } else {
                    use rsa::pkcs8::DecodePublicKey as _;
                    let public = rsa::RsaPublicKey::from_public_key_der(&bytes(&key)?)
                        .map_err(|error| format!("not a public key Raygun reads: {error}"))?;
                    json!({
                        "kind": "RSA",
                        "modulus": public.n().to_str_radix(16),
                        "exponent": public.e().to_str_radix(16),
                    })
                }
            } else if let Ok(secret) = ec_secret(&key) {
                json!({ "kind": "EC", "s": hex(&secret.to_bytes()) })
            } else {
                let private = rsa::RsaPrivateKey::from_pkcs8_der(&bytes(&key)?)
                    .map_err(|error| format!("not a private key Raygun reads: {error}"))?;
                json!({
                    "kind": "RSA",
                    "modulus": private.n().to_str_radix(16),
                    "exponent": private.e().to_str_radix(16),
                })
            }
        }
        KeyArgs::EcPublic { x, y } => {
            let mut point = vec![4u8];
            point.extend(hex_bytes(&x, 32)?);
            point.extend(hex_bytes(&y, 32)?);
            let public = p256::PublicKey::from_sec1_bytes(&point)
                .map_err(|_| "the point isn't on P-256".to_owned())?;
            json!({ "public": ec_public_der(&public)? })
        }
        KeyArgs::RsaPublic { modulus, exponent } => {
            let number = |text: &str| {
                rsa::BigUint::parse_bytes(text.trim_start_matches('-').as_bytes(), 16)
                    .ok_or_else(|| "not hexadecimal".to_owned())
            };
            let public = rsa::RsaPublicKey::new(number(&modulus)?, number(&exponent)?)
                .map_err(|error| error.to_string())?;
            json!({ "public": encode(public.to_public_key_der().map_err(|error| error.to_string())?.as_bytes()) })
        }
        KeyArgs::Agree { private, public } => {
            let secret = ec_secret(&private)?;
            let public = ec_public(&public)?;
            let shared = p256::ecdh::diffie_hellman(secret.to_nonzero_scalar(), public.as_affine());
            json!({ "data": encode(shared.raw_secret_bytes()) })
        }
        KeyArgs::Sign { private, data } => {
            use p256::ecdsa::signature::Signer as _;
            let signing = p256::ecdsa::SigningKey::from(&ec_secret(&private)?);
            let signature: p256::ecdsa::Signature = signing.sign(&bytes(&data)?);
            json!({ "data": encode(signature.to_der().as_bytes()) })
        }
        KeyArgs::Verify {
            public,
            data,
            signature,
        } => {
            use p256::ecdsa::signature::Verifier as _;
            let verifying = p256::ecdsa::VerifyingKey::from(&ec_public(&public)?);
            let signature = p256::ecdsa::Signature::from_der(&bytes(&signature)?)
                .map_err(|error| error.to_string())?;
            json!({ "valid": verifying.verify(&bytes(&data)?, &signature).is_ok() })
        }
        KeyArgs::Rsa {
            key,
            encrypt,
            oaep,
            hash,
            mgf_hash,
            data,
        } => {
            let data = bytes(&data)?;
            let out = rsa_run(&key, encrypt, oaep, &hash, &mgf_hash, &data)?;
            json!({ "data": encode(out) })
        }
    };
    Ok(answer.to_string())
}

/// RSA as Java's `Cipher` does it: `RSA/ECB/PKCS1Padding`, `OAEPWithSHA-256AndMGF1Padding` (its
/// mask over SHA-1 unless the parameters say otherwise, as Java's is).
fn rsa_run(
    key: &str,
    encrypt: bool,
    oaep: bool,
    hash: &str,
    mgf_hash: &str,
    data: &[u8],
) -> Result<Vec<u8>, String> {
    use rsa::pkcs8::DecodePublicKey as _;
    use rsa::sha2::{Sha256, Sha384, Sha512};
    let name = |text: &str| text.to_ascii_lowercase().replace(['-', '_'], "");
    let padding = || -> Result<rsa::Oaep, String> {
        Ok(match (name(hash).as_str(), name(mgf_hash).as_str()) {
            ("sha256", "sha256") => rsa::Oaep::new::<Sha256>(),
            ("sha256", _) => rsa::Oaep::new_with_mgf_hash::<Sha256, sha1_10::Sha1>(),
            ("sha384", _) => rsa::Oaep::new::<Sha384>(),
            ("sha512", _) => rsa::Oaep::new::<Sha512>(),
            ("sha1" | "", _) => rsa::Oaep::new::<sha1_10::Sha1>(),
            (other, _) => return Err(format!("no OAEP with {other}")),
        })
    };
    let failed = |error: rsa::Error| format!("RSA failed: {error}");
    let der = bytes(key)?;
    if encrypt {
        let public = rsa::RsaPublicKey::from_public_key_der(&der)
            .or_else(|_| {
                rsa::RsaPrivateKey::from_pkcs8_der(&der)
                    .map(|private| rsa::RsaPublicKey::from(&private))
            })
            .map_err(|error| format!("not an RSA key: {error}"))?;
        if oaep {
            public.encrypt(&mut OsRng, padding()?, data).map_err(failed)
        } else {
            public
                .encrypt(&mut OsRng, rsa::Pkcs1v15Encrypt, data)
                .map_err(failed)
        }
    } else {
        let private = rsa::RsaPrivateKey::from_pkcs8_der(&der)
            .map_err(|error| format!("not an RSA private key: {error}"))?;
        if oaep {
            private.decrypt(padding()?, data).map_err(failed)
        } else {
            private.decrypt(rsa::Pkcs1v15Encrypt, data).map_err(failed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(args: Value) -> Value {
        serde_json::from_str(&run(&args.to_string()).unwrap()).unwrap()
    }

    #[test]
    fn ec_keys_agree_sign_and_rebuild_from_their_point() {
        let alice = call(json!({ "op": "generate", "algorithm": "EC" }));
        let bob = call(json!({ "op": "generate", "algorithm": "EC" }));
        let one =
            call(json!({ "op": "agree", "private": alice["private"], "public": bob["public"] }));
        let two =
            call(json!({ "op": "agree", "private": bob["private"], "public": alice["public"] }));
        assert_eq!(one["data"], two["data"]);
        let point = call(json!({ "op": "describe", "key": alice["public"], "public": true }));
        assert_eq!(point["kind"], "EC");
        let rebuilt = call(json!({ "op": "ecPublic", "x": point["x"], "y": point["y"] }));
        assert_eq!(rebuilt["public"], alice["public"]);
        let data = encode(b"signed");
        let signature = call(json!({ "op": "sign", "private": alice["private"], "data": data }));
        let checked = call(json!({
            "op": "verify",
            "public": alice["public"],
            "data": data,
            "signature": signature["data"],
        }));
        assert_eq!(checked["valid"], true);
    }

    #[test]
    fn rsa_keys_encrypt_and_decrypt_both_paddings() {
        let pair = call(json!({ "op": "generate", "algorithm": "RSA", "bits": 1024 }));
        for (oaep, hash, mgf) in [
            (false, "", ""),
            (true, "SHA-256", "SHA-256"),
            (true, "SHA-256", ""),
        ] {
            let sealed = call(json!({
                "op": "rsa", "key": pair["public"], "encrypt": true, "oaep": oaep,
                "hash": hash, "mgfHash": mgf, "data": encode(b"secret"),
            }));
            let opened = call(json!({
                "op": "rsa", "key": pair["private"], "encrypt": false, "oaep": oaep,
                "hash": hash, "mgfHash": mgf, "data": sealed["data"],
            }));
            assert_eq!(opened["data"], encode(b"secret"));
        }
        let parts = call(json!({ "op": "describe", "key": pair["public"], "public": true }));
        let rebuilt = call(
            json!({ "op": "rsaPublic", "modulus": parts["modulus"], "exponent": parts["exponent"] }),
        );
        assert_eq!(rebuilt["public"], pair["public"]);
    }
}
