//! The runtime `mihon`: a converted extension runs in Raygun's QuickJS host (`js.rs`) after the
//! prelude (`prelude.js`) and the themes (`themes/*.js`), which give it what Mihon's extensions
//! lean on: Jsoup's documents (Raygun's Jsoup-compatible selector engine through the LNReader
//! runtime's document store), Kotlin's helpers, OkHttp-like requests over `raygun.http`, Java's
//! date formats and, for the themes that need it, OpenSSL-style AES.

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine as _;
use md5::{Digest, Md5};
use rquickjs::{Ctx, Function, Object};
use serde_json::json;

use crate::js::{err, eval_script, ok};
use crate::lnreader::dom::Documents;

const PRELUDE: &str = include_str!("prelude.js");
/// The runtime of sources converted from Kotlin (`crate::mihon::transpile`).
const KOTLIN: &str = include_str!("kotlin.js");
/// The themes, each registering itself with the prelude's `mihon.defineTheme`.
const THEMES: [(&str, &str); 2] = [
    ("mihon:madara", include_str!("themes/madara.js")),
    (
        "mihon:mangathemesia",
        include_str!("themes/mangathemesia.js"),
    ),
];

/// A converted extension ready to evaluate.
pub(crate) struct Prepared {
    code: String,
}

impl Prepared {
    pub fn new(code: &str) -> Result<Self, String> {
        if !code.contains("mihon.") {
            return Err("This isn't a converted Mihon extension".into());
        }
        Ok(Self {
            code: code.to_owned(),
        })
    }
}

/// Installs the natives, evaluates the prelude and the themes, then the extension.
pub(crate) fn install(ctx: &Ctx<'_>, prepared: &Prepared, filename: &str) -> Result<(), String> {
    natives(ctx).map_err(|error| error.to_string())?;
    eval_script(ctx, PRELUDE, "mihon:prelude", true)?;
    eval_script(ctx, KOTLIN, "mihon:kotlin", true)?;
    for (name, theme) in THEMES {
        eval_script(ctx, theme, name, true)?;
    }
    eval_script(ctx, &prepared.code, filename, true)
        .map_err(|error| format!("The converted extension stopped with an error: {error}"))
}

fn envelope<T: serde::Serialize>(result: Result<T, String>) -> String {
    match result {
        Ok(value) => ok(value),
        Err(message) => err(message),
    }
}

fn natives(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;
    let documents = std::rc::Rc::new(Documents::default());
    native.set(
        "dom",
        Function::new(ctx.clone(), move |op: String, args: String| {
            envelope(documents.run(&op, &args))
        })?,
    )?;
    native.set(
        "url",
        Function::new(ctx.clone(), |href: String, base: String| {
            envelope(parse_url(&href, &base))
        })?,
    )?;
    native.set(
        "decrypt",
        Function::new(ctx.clone(), |args: String| envelope(decrypt(&args)))?,
    )?;
    ctx.globals().set("__rgMihon", native)
}

/// An address and its parts, resolved against `base` when it's relative.
fn parse_url(href: &str, base: &str) -> Result<serde_json::Value, String> {
    let href = href.trim();
    let url = if base.is_empty() {
        url::Url::parse(href)
    } else {
        url::Url::parse(base).and_then(|base| base.join(href))
    }
    .map_err(|error| format!("Invalid URL {href:?}: {error}"))?;
    let host = url.host_str().unwrap_or_default().to_owned();
    Ok(json!({
        "href": url.as_str(),
        "scheme": url.scheme(),
        "host": host,
        "port": url.port(),
        "path": url.path(),
        "query": url.query(),
        "fragment": url.fragment(),
        "origin": url.origin().ascii_serialization(),
    }))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecryptArgs {
    /// The ciphertext, base64.
    data: String,
    /// A passphrase whose key and IV come from OpenSSL's `EVP_BytesToKey` with MD5 (CryptoJS's
    /// `AES.decrypt(text, passphrase)`), with `salt` (hex) or the `Salted__` header's.
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    salt: Option<String>,
    /// Or the key and IV themselves, hex.
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    iv: Option<String>,
}

fn hex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd-length hex".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|_| "invalid hex".to_owned()))
        .collect()
}

/// AES-CBC decryption (128, 192 or 256 bits by the key), as text.
fn decrypt(args: &str) -> Result<String, String> {
    let args: DecryptArgs =
        serde_json::from_str(args).map_err(|error| format!("invalid arguments: {error}"))?;
    let engine = base64::engine::general_purpose::STANDARD;
    let mut data = engine
        .decode(args.data.trim())
        .map_err(|error| format!("the ciphertext isn't base64: {error}"))?;
    let (key, iv) = match (args.key, args.iv, args.password) {
        (Some(key), Some(iv), _) => (hex(&key)?, hex(&iv)?),
        (_, _, Some(password)) => {
            let salt = match args.salt {
                Some(salt) => hex(&salt)?,
                None if data.starts_with(b"Salted__") && data.len() >= 16 => {
                    let salt = data[8..16].to_vec();
                    data.drain(..16);
                    salt
                }
                None => Vec::new(),
            };
            // EVP_BytesToKey: MD5 rounds over the previous digest, the passphrase and the salt.
            let mut derived = Vec::new();
            let mut previous: Vec<u8> = Vec::new();
            while derived.len() < 48 {
                let mut hasher = Md5::new();
                hasher.update(&previous);
                hasher.update(password.as_bytes());
                hasher.update(&salt);
                previous = hasher.finalize().to_vec();
                derived.extend_from_slice(&previous);
            }
            (derived[..32].to_vec(), derived[32..48].to_vec())
        }
        _ => return Err("decrypt needs a key and an IV, or a password".into()),
    };
    let plain = match key.len() {
        16 => cbc::Decryptor::<aes::Aes128>::new_from_slices(&key, &iv)
            .map_err(|error| error.to_string())?
            .decrypt_padded_vec_mut::<Pkcs7>(&data),
        24 => cbc::Decryptor::<aes::Aes192>::new_from_slices(&key, &iv)
            .map_err(|error| error.to_string())?
            .decrypt_padded_vec_mut::<Pkcs7>(&data),
        32 => cbc::Decryptor::<aes::Aes256>::new_from_slices(&key, &iv)
            .map_err(|error| error.to_string())?
            .decrypt_padded_vec_mut::<Pkcs7>(&data),
        other => return Err(format!("a {other}-byte AES key isn't one")),
    }
    .map_err(|_| "the data didn't decrypt (wrong key or padding)".to_owned())?;
    Ok(String::from_utf8_lossy(&plain).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypts_openssl_style_ciphertext() {
        // `echo -n '["https://img.example/1.jpg"]' | openssl enc -aes-256-cbc -md md5 -pass
        // pass:secret -S 0102030405060708 -base64` (no `Salted__` header, salt given apart).
        use aes::cipher::BlockEncryptMut;
        let (password, salt) = ("secret", [1u8, 2, 3, 4, 5, 6, 7, 8]);
        let mut derived = Vec::new();
        let mut previous: Vec<u8> = Vec::new();
        while derived.len() < 48 {
            let mut hasher = Md5::new();
            hasher.update(&previous);
            hasher.update(password.as_bytes());
            hasher.update(salt);
            previous = hasher.finalize().to_vec();
            derived.extend_from_slice(&previous);
        }
        let plain = br#"["https://img.example/1.jpg"]"#;
        let cipher =
            cbc::Encryptor::<aes::Aes256>::new_from_slices(&derived[..32], &derived[32..48])
                .unwrap()
                .encrypt_padded_vec_mut::<Pkcs7>(plain);
        let data = base64::engine::general_purpose::STANDARD.encode(cipher);
        let args = json!({ "data": data, "password": password, "salt": "0102030405060708" });
        assert_eq!(
            decrypt(&args.to_string()).unwrap(),
            String::from_utf8_lossy(plain)
        );
        let wrong = json!({ "data": data, "password": "nope", "salt": "0102030405060708" });
        assert!(decrypt(&wrong.to_string()).is_err());
    }

    #[test]
    fn urls_resolve_against_their_page() {
        let url = parse_url("../ch-2/", "https://site.example/manga/one/ch-1/").unwrap();
        assert_eq!(url["href"], "https://site.example/manga/one/ch-2/");
        assert_eq!(url["path"], "/manga/one/ch-2/");
        assert_eq!(url["host"], "site.example");
        assert!(parse_url("not a url", "").is_err());
    }
}
