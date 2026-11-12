//! Running a Nuvio provider in Raygun's QuickJS runtime (`js.rs`) as Nuvio runs it
//! (tapframe/NuvioStreaming, GPL-3.0: `PluginRuntime.kt` wraps the script as a CommonJS module,
//! `JsBindings.kt` gives the globals). The web the provider finds (timers, base64, `URL`,
//! `Headers`, cheerio) is the LNReader runtime's (`crate::lnreader`), evaluated first; the
//! Nuvio prelude adds the rest (its `fetch`, crypto, `TextEncoder`, `require`, the settings
//! globals) and presents the provider as a `video.streams` provider. crypto-js is evaluated for
//! the providers that use it, as for Miru's scripts.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Function, Object};
use serde_json::json;

use crate::js::{err, eval_script, ok};
use crate::manifest::SettingDef;

const PRELUDE: &str = include_str!("prelude.js");
/// What crypto-js 3 lacks that Nuvio's own CryptoJS has: `enc.Base64url`.
const CRYPTO_ADDITIONS: &str = r"(() => {
  const C = globalThis.CryptoJS;
  if (!C || C.enc.Base64url) return;
  C.enc.Base64url = {
    stringify: (words) => C.enc.Base64.stringify(words).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, ''),
    parse: (text) => {
      let base64 = String(text || '').replace(/-/g, '+').replace(/_/g, '/');
      while (base64.length % 4) base64 += '=';
      return C.enc.Base64.parse(base64);
    },
  };
})();";
/// Most random bytes one `getRandomValues` asks for (the Web Crypto API's own limit).
const MAX_RANDOM_BYTES: usize = 65_536;

/// A provider ready to evaluate: its code inside Nuvio's wrapper, then the start-up.
pub(crate) struct Prepared {
    code: String,
    crypto: bool,
}

impl Prepared {
    /// `meta` is the manifest's `nuvio` section: the provider's id and the types it serves.
    pub fn new(code: &str, meta: Option<&serde_json::Value>) -> Result<Self, String> {
        if !code.contains("getStreams") {
            return Err(crate::purpose::message(super::package::NO_GET_STREAMS));
        }
        let meta = json!({
            "scraper": meta.and_then(|meta| meta.get("scraper")).cloned().unwrap_or_default(),
            "types": meta
                .and_then(|meta| meta.get("types"))
                .cloned()
                .unwrap_or_else(|| json!(["movie", "tv"])),
        });
        Ok(Self {
            crypto: code.contains("CryptoJS") || code.contains("crypto-js"),
            code: format!(
                "__nuvioSetup({meta});\nglobalThis.__rg_ready = __nuvioStart((function (module, exports, require) {{\n{code}\n;return module;\n}}).call(globalThis, __nuvioModule, __nuvioModule.exports, require));\n"
            ),
        })
    }
}

/// What the natives share with the rest of the extension.
pub(crate) struct Hooks {
    pub settings: Rc<RefCell<Vec<SettingDef>>>,
}

/// Installs the natives, evaluates the preludes, then the provider and its start-up.
pub(crate) fn install(
    ctx: &Ctx<'_>,
    prepared: &Prepared,
    hooks: Hooks,
    filename: &str,
) -> Result<(), String> {
    crate::lnreader::runtime::install_web(
        ctx,
        crate::lnreader::runtime::Hooks {
            settings: hooks.settings.clone(),
        },
    )?;
    natives(ctx, hooks).map_err(|error| error.to_string())?;
    eval_script(ctx, PRELUDE, "nuvio:prelude", false)?;
    if prepared.crypto {
        eval_script(
            ctx,
            crate::miru::runtime::CRYPTO_JS,
            "nuvio:crypto-js",
            false,
        )?;
        eval_script(ctx, CRYPTO_ADDITIONS, "nuvio:crypto-additions", false)?;
    }
    eval_script(ctx, &prepared.code, filename, false)
        .map_err(|error| format!("The Nuvio provider stopped with an error: {error}"))
}

fn envelope(result: Result<serde_json::Value, String>) -> String {
    match result {
        Ok(value) => ok(value),
        Err(message) => err(message),
    }
}

/// `__rgNuvio`, taken and removed by the prelude: digests, HMACs, PBKDF2 and AES (bytes as
/// base64, `mihon::runtime::crypto`), random bytes, and the settings `onSettings` declares.
fn natives(ctx: &Ctx<'_>, hooks: Hooks) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;
    native.set(
        "crypto",
        Function::new(ctx.clone(), |args: String| {
            envelope(crate::mihon::runtime::crypto(&args).map(serde_json::Value::String))
        })?,
    )?;
    native.set(
        "random",
        Function::new(ctx.clone(), |count: f64| {
            use base64::Engine as _;
            let count = if count.is_finite() {
                (count.max(0.0) as usize).min(MAX_RANDOM_BYTES)
            } else {
                0
            };
            let mut bytes = vec![0; count];
            envelope(
                getrandom::fill(&mut bytes)
                    .map(|()| {
                        serde_json::Value::String(
                            base64::engine::general_purpose::STANDARD.encode(&bytes),
                        )
                    })
                    .map_err(|error| format!("no random bytes: {error}")),
            )
        })?,
    )?;
    let settings = hooks.settings;
    native.set(
        "registerSetting",
        Function::new(ctx.clone(), move |setting: String| {
            envelope(
                crate::lnreader::runtime::register_setting(&settings, &setting)
                    .map(|()| serde_json::Value::Null),
            )
        })?,
    )?;
    ctx.globals().set("__rgNuvio", native)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_the_provider_as_nuvio_does() {
        let prepared = Prepared::new(
            "module.exports = { getStreams };",
            Some(&json!({ "scraper": "archive", "types": ["movie"] })),
        )
        .unwrap();
        assert!(
            prepared
                .code
                .contains("function (module, exports, require)")
        );
        assert!(
            prepared
                .code
                .starts_with(r#"__nuvioSetup({"scraper":"archive","types":["movie"]});"#)
        );
        assert!(!prepared.crypto);
        assert!(
            Prepared::new("const CryptoJS = require('crypto-js'); getStreams", None)
                .unwrap()
                .crypto
        );
        assert!(
            Prepared::new("module.exports = {};", None)
                .err()
                .unwrap()
                .starts_with("Not for Raygun")
        );
    }
}
