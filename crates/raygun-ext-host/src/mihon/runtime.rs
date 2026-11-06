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

use crate::cookies::CookieJar;
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

/// What the runtime reaches of the extension's host: its cookie jar (`client.cookieJar`).
pub(crate) struct Hooks {
    pub cookies: std::sync::Arc<CookieJar>,
}

/// Installs the natives, evaluates the prelude and the themes, then the extension.
pub(crate) fn install(
    ctx: &Ctx<'_>,
    prepared: &Prepared,
    hooks: Hooks,
    filename: &str,
) -> Result<(), String> {
    natives(ctx, hooks).map_err(|error| error.to_string())?;
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

fn natives(ctx: &Ctx<'_>, hooks: Hooks) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;
    super::pictures::install(ctx, &native)?;
    // OkHttp's `cookieJar.loadForRequest(url)`: the cookies the jar sends there, as
    // `name=value` pairs; `saveFromResponse(url, cookies)`: `Set-Cookie` lines kept for it.
    let jar = hooks.cookies.clone();
    native.set(
        "cookies",
        Function::new(ctx.clone(), move |url: String| {
            envelope(cookies_for(&jar, &url))
        })?,
    )?;
    let jar = hooks.cookies;
    native.set(
        "setCookies",
        Function::new(ctx.clone(), move |url: String, lines: String| {
            envelope(set_cookies(&jar, &url, &lines))
        })?,
    )?;
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
    native.set(
        "crypto",
        Function::new(ctx.clone(), |args: String| envelope(crypto(&args)))?,
    )?;
    native.set(
        "evaluate",
        Function::new(ctx.clone(), |code: String| envelope(evaluate(&code)))?,
    )?;
    ctx.globals().set("__rgMihon", native)
}

/// What Keiyoushi's crypto helpers (and Java's `Cipher`, `MessageDigest`, `Mac`,
/// `SecretKeyFactory` as sources call them) ask of the runtime. Bytes go as base64.
#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum CryptoArgs {
    /// AES in `mode` (`cbc`, `ecb`, `gcm`, `ctr`).
    #[serde(rename_all = "camelCase")]
    Aes {
        mode: String,
        key: String,
        #[serde(default)]
        iv: Option<String>,
        #[serde(default = "yes")]
        padding: bool,
        #[serde(default)]
        encrypt: bool,
        data: String,
    },
    /// A digest (`md5`, `sha1`, `sha256`, `sha512`).
    Digest { algorithm: String, data: String },
    /// An HMAC with the digest `algorithm`.
    Hmac {
        algorithm: String,
        key: String,
        data: String,
    },
    /// PBKDF2 with HMAC over the digest `algorithm`: `length` bytes.
    Pbkdf2 {
        algorithm: String,
        password: String,
        salt: String,
        iterations: u32,
        length: usize,
    },
}

fn yes() -> bool {
    true
}

fn crypto(args: &str) -> Result<String, String> {
    let args: CryptoArgs =
        serde_json::from_str(args).map_err(|error| format!("invalid arguments: {error}"))?;
    let engine = base64::engine::general_purpose::STANDARD;
    let bytes = |text: &str| {
        engine
            .decode(text)
            .map_err(|error| format!("not base64: {error}"))
    };
    let out = match args {
        CryptoArgs::Aes {
            mode,
            key,
            iv,
            padding,
            encrypt,
            data,
        } => aes_run(
            &mode,
            &bytes(&key)?,
            iv.as_deref().map(bytes).transpose()?.as_deref(),
            padding,
            encrypt,
            &bytes(&data)?,
        )?,
        CryptoArgs::Digest { algorithm, data } => digest(&algorithm, &bytes(&data)?)?,
        CryptoArgs::Hmac {
            algorithm,
            key,
            data,
        } => hmac_of(&algorithm, &bytes(&key)?, &bytes(&data)?)?,
        CryptoArgs::Pbkdf2 {
            algorithm,
            password,
            salt,
            iterations,
            length,
        } => pbkdf2(
            &algorithm,
            &bytes(&password)?,
            &bytes(&salt)?,
            iterations,
            length,
        )?,
    };
    Ok(engine.encode(out))
}

fn digest(algorithm: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    use sha2::Digest as _;
    Ok(
        match algorithm.to_ascii_lowercase().replace('-', "").as_str() {
            "md5" => Md5::digest(data).to_vec(),
            "sha1" => sha1::Sha1::digest(data).to_vec(),
            "sha256" => sha2::Sha256::digest(data).to_vec(),
            "sha384" => sha2::Sha384::digest(data).to_vec(),
            "sha512" => sha2::Sha512::digest(data).to_vec(),
            other => return Err(format!("no {other} digest")),
        },
    )
}

fn hmac_of(algorithm: &str, key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    use hmac::{KeyInit as _, Mac as _};
    macro_rules! run {
        ($digest:ty) => {{
            let mut mac =
                hmac::Hmac::<$digest>::new_from_slice(key).map_err(|error| error.to_string())?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }};
    }
    let name = algorithm.to_ascii_lowercase().replace(['-', '_'], "");
    let name = name.trim_start_matches("hmac");
    Ok(match name {
        "md5" => run!(Md5),
        "sha1" => run!(sha1::Sha1),
        "sha256" => run!(sha2::Sha256),
        "sha384" => run!(sha2::Sha384),
        "sha512" => run!(sha2::Sha512),
        other => return Err(format!("no HMAC with {other}")),
    })
}

/// PBKDF2 (RFC 8018) over HMAC.
fn pbkdf2(
    algorithm: &str,
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    length: usize,
) -> Result<Vec<u8>, String> {
    let name = algorithm
        .to_ascii_lowercase()
        .replace("pbkdf2with", "")
        .replace("hmac", "");
    let mut out = Vec::new();
    let mut block = 1u32;
    while out.len() < length {
        let mut first = salt.to_vec();
        first.extend_from_slice(&block.to_be_bytes());
        let mut u = hmac_of(&name, password, &first)?;
        let mut t = u.clone();
        for _ in 1..iterations.max(1) {
            u = hmac_of(&name, password, &u)?;
            for (byte, next) in t.iter_mut().zip(&u) {
                *byte ^= next;
            }
        }
        out.extend_from_slice(&t);
        block += 1;
    }
    out.truncate(length);
    Ok(out)
}

fn aes_run(
    mode: &str,
    key: &[u8],
    iv: Option<&[u8]>,
    padding: bool,
    encrypt: bool,
    data: &[u8],
) -> Result<Vec<u8>, String> {
    use aes::cipher::block_padding::NoPadding;
    use aes::cipher::{BlockDecrypt, BlockEncrypt, BlockEncryptMut, KeyInit, StreamCipher};
    let wrong = |_| "the data didn't decrypt (wrong key or padding)".to_owned();
    let iv_of = || iv.ok_or_else(|| "this AES mode needs an IV".to_owned());
    macro_rules! by_key {
        ($run:ident) => {
            match key.len() {
                16 => $run!(aes::Aes128),
                24 => $run!(aes::Aes192),
                32 => $run!(aes::Aes256),
                other => return Err(format!("a {other}-byte AES key isn't one")),
            }
        };
    }
    Ok(match mode {
        "cbc" => {
            let iv = iv_of()?;
            macro_rules! cbc_run {
                ($aes:ty) => {{
                    if encrypt {
                        let cipher = cbc::Encryptor::<$aes>::new_from_slices(key, iv)
                            .map_err(|error| error.to_string())?;
                        if padding {
                            cipher.encrypt_padded_vec_mut::<Pkcs7>(data)
                        } else {
                            cipher.encrypt_padded_vec_mut::<NoPadding>(data)
                        }
                    } else {
                        let cipher = cbc::Decryptor::<$aes>::new_from_slices(key, iv)
                            .map_err(|error| error.to_string())?;
                        if padding {
                            cipher
                                .decrypt_padded_vec_mut::<Pkcs7>(data)
                                .map_err(wrong)?
                        } else {
                            cipher
                                .decrypt_padded_vec_mut::<NoPadding>(data)
                                .map_err(wrong)?
                        }
                    }
                }};
            }
            by_key!(cbc_run)
        }
        "ecb" => {
            macro_rules! ecb_run {
                ($aes:ty) => {{
                    let cipher = <$aes>::new_from_slice(key).map_err(|error| error.to_string())?;
                    let mut input = data.to_vec();
                    if encrypt && padding {
                        let pad = 16 - input.len() % 16;
                        input.extend(std::iter::repeat_n(pad as u8, pad));
                    }
                    if input.len() % 16 != 0 {
                        return Err("the data isn't whole AES blocks".into());
                    }
                    for chunk in input.chunks_mut(16) {
                        let block = aes::cipher::generic_array::GenericArray::from_mut_slice(chunk);
                        if encrypt {
                            cipher.encrypt_block(block);
                        } else {
                            cipher.decrypt_block(block);
                        }
                    }
                    if !encrypt && padding {
                        let pad = usize::from(*input.last().ok_or("no data")?);
                        if pad == 0 || pad > 16 || pad > input.len() {
                            return Err("the data didn't decrypt (wrong key or padding)".into());
                        }
                        input.truncate(input.len() - pad);
                    }
                    input
                }};
            }
            by_key!(ecb_run)
        }
        "ctr" => {
            let iv = iv_of()?;
            macro_rules! ctr_run {
                ($aes:ty) => {{
                    use aes::cipher::KeyIvInit as _;
                    let mut cipher = ctr::Ctr128BE::<$aes>::new_from_slices(key, iv)
                        .map_err(|error| error.to_string())?;
                    let mut out = data.to_vec();
                    cipher.apply_keystream(&mut out);
                    out
                }};
            }
            by_key!(ctr_run)
        }
        "gcm" => {
            use aes_gcm::aead::{Aead, KeyInit as _, generic_array::GenericArray};
            let iv = iv_of()?;
            if iv.len() != 12 {
                return Err("GCM takes a 12-byte IV".into());
            }
            let nonce = GenericArray::from_slice(iv);
            macro_rules! gcm_run {
                ($cipher:ty) => {{
                    let cipher =
                        <$cipher>::new_from_slice(key).map_err(|error| error.to_string())?;
                    if encrypt {
                        cipher
                            .encrypt(nonce, data)
                            .map_err(|_| "couldn't encrypt".to_owned())?
                    } else {
                        cipher
                            .decrypt(nonce, data)
                            .map_err(|_| "the data didn't decrypt (wrong key or tag)".to_owned())?
                    }
                }};
            }
            match key.len() {
                16 => gcm_run!(aes_gcm::Aes128Gcm),
                32 => gcm_run!(aes_gcm::Aes256Gcm),
                other => return Err(format!("a {other}-byte AES-GCM key isn't one")),
            }
        }
        other => return Err(format!("no AES mode {other}")),
    })
}

/// `QuickJs.create().evaluate(script)` (a site's script a source runs for a key or a list):
/// in a JavaScript engine of its own, with nothing of the extension's, at most 5 s and 64 MB.
/// The script's value comes back as JSON (`null` when it has none).
fn evaluate(code: &str) -> Result<serde_json::Value, String> {
    let runtime = rquickjs::Runtime::new().map_err(|error| error.to_string())?;
    runtime.set_memory_limit(64 * 1024 * 1024);
    runtime.set_max_stack_size(1024 * 1024);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    runtime.set_interrupt_handler(Some(Box::new(move || std::time::Instant::now() > deadline)));
    let context = rquickjs::Context::full(&runtime).map_err(|error| error.to_string())?;
    context.with(|ctx| {
        let value: rquickjs::Value = match ctx.eval(code) {
            Ok(value) => value,
            Err(rquickjs::Error::Exception) => {
                let thrown = ctx.catch();
                let message = thrown
                    .as_exception()
                    .and_then(|exception| exception.message())
                    .unwrap_or_else(|| "the script failed".to_owned());
                return Err(format!("the script failed: {message}"));
            }
            Err(error) => return Err(error.to_string()),
        };
        let text = ctx
            .json_stringify(value)
            .map_err(|error| error.to_string())?
            .map(|text| text.to_string())
            .transpose()
            .map_err(|error| error.to_string())?;
        Ok(text
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(serde_json::Value::Null))
    })
}

/// The cookies the jar sends to `url`, as `[name, value]` pairs.
fn cookies_for(jar: &CookieJar, url: &str) -> Result<Vec<(String, String)>, String> {
    use reqwest::cookie::CookieStore as _;
    let url = reqwest::Url::parse(url).map_err(|error| format!("Invalid URL {url:?}: {error}"))?;
    let header = jar
        .cookies(&url)
        .and_then(|value| value.to_str().ok().map(str::to_owned))
        .unwrap_or_default();
    Ok(header
        .split(';')
        .filter_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            Some((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect())
}

/// Keeps `Set-Cookie` lines (a JSON list) in the jar as if `url` had answered with them.
fn set_cookies(jar: &CookieJar, url: &str, lines: &str) -> Result<(), String> {
    use reqwest::cookie::CookieStore as _;
    let url = reqwest::Url::parse(url).map_err(|error| format!("Invalid URL {url:?}: {error}"))?;
    let lines: Vec<String> =
        serde_json::from_str(lines).map_err(|error| format!("invalid cookies: {error}"))?;
    let values: Vec<reqwest::header::HeaderValue> = lines
        .iter()
        .filter_map(|line| reqwest::header::HeaderValue::from_str(line).ok())
        .collect();
    jar.set_cookies(&mut values.iter(), &url);
    Ok(())
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
