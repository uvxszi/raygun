//! A script's failure in words a reader understands: which source, what went wrong, what they can
//! do. The technical detail (the HTTP status, the Lua error) follows in parentheses, for "More
//! info"; a Lua traceback never reaches the app.

use super::http::FetchError;

/// Lua errors that mean a page no longer looks the way the script expects: a selector found
/// nothing, a field is missing, a selector built from the page's text doesn't parse.
const PAGE_CHANGED: [&str; 16] = [
    "attempt to index",
    "attempt to call",
    "attempt to perform arithmetic",
    "attempt to concatenate",
    "attempt to compare",
    "attempt to get length",
    "bad argument #",
    "table index is nil",
    "invalid value (at index",
    "error converting lua",
    "expected a url",
    "string expected, got",
    "number expected, got",
    "invalid url",
    "in selector",
    "attribute selector",
];

/// Errors from parts of Shosetsu's API Raygun doesn't have.
const UNSUPPORTED: [&str; 2] = ["isn't supported in raygun", "unsupported pseudo-class"];

/// The first line of an error, without Lua's `[string "ext(...)"]:12:` position.
pub fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    let line = line.trim_start_matches("runtime error: ").trim();
    strip_position(line).to_owned()
}

fn strip_position(mut line: &str) -> &str {
    // `[string "ext(x)"]:617: ` or `prelude:157: `, possibly more than one.
    loop {
        let rest = if line.starts_with("[string ") {
            line.find("]:").map(|end| &line[end + 2..])
        } else {
            None
        };
        let Some(rest) = rest else {
            return line;
        };
        let digits = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        match digits.strip_prefix(':') {
            Some(after) if digits.len() < rest.len() => line = after.trim_start(),
            _ => return line,
        }
    }
}

/// The typed request failure inside a Lua error, if a request is what failed.
fn fetch_error(error: &mlua::Error) -> Option<&FetchError> {
    match error {
        mlua::Error::ExternalError(inner) => inner.downcast_ref::<FetchError>(),
        mlua::Error::CallbackError { cause, .. } => fetch_error(cause),
        mlua::Error::WithContext { cause, .. } => fetch_error(cause),
        _ => None,
    }
}

fn is_dns(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    [
        "no such host",
        "name or service not known",
        "nodename nor servname",
        "failed to lookup",
        "doesn't resolve",
        "dns error",
        "os error 11001",
        "os error 11002",
        "no address associated",
        "temporary failure in name resolution",
    ]
    .iter()
    .any(|marker| detail.contains(marker))
}

/// A request's failure, for source `name`.
pub fn fetch_message(name: &str, error: &FetchError) -> String {
    let detail = error.to_string();
    let sentence = match error {
        FetchError::BrowserCheck { .. } => {
            format!("{name} wants a browser check that Raygun can't do. Try again later")
        }
        FetchError::Status { status, .. } => match status {
            404 | 410 => format!("{name} changed its site: a page it reads is gone"),
            401 | 403 => format!("{name} refused Raygun's request"),
            429 => format!("{name} is limiting requests. Try again in a minute"),
            500..=599 => format!("{name} has a problem on its side. Try again later"),
            _ => format!("{name} answered with an error"),
        },
        FetchError::Timeout { .. } => format!("{name} didn't answer in time"),
        FetchError::Unreachable { detail, .. } if is_dns(detail) => {
            format!("{name}'s site can't be found; it may have moved or closed")
        }
        FetchError::Unreachable { .. } => format!("{name} didn't answer"),
        FetchError::TooLarge { .. } => format!("{name} sent a page too large to read"),
        FetchError::NotPublic { .. } => {
            format!(
                "{name} tried to reach this device or the local network, which sources may not do"
            )
        }
        FetchError::BadUrl { .. } => format!("{name} changed its site"),
    };
    format!("{sentence} ({detail})")
}

/// A failed call or load of source `name`. `challenged` is the host of a bot check seen meanwhile:
/// the script usually fails on the check's page rather than saying so.
pub fn describe(name: &str, error: &mlua::Error, challenged: Option<&str>) -> String {
    if let Some(fetch) = fetch_error(error) {
        return fetch_message(name, fetch);
    }
    if let mlua::Error::MemoryError(_) = error {
        return format!("{name} sent more than Raygun can hold (not enough memory)");
    }
    // Lua that LuaJ took and Lua 5.2 doesn't, most likely.
    if let mlua::Error::SyntaxError { message, .. } = error {
        return format!(
            "Raygun can't run part of {name} yet ({})",
            first_line(message)
        );
    }
    let detail = match error {
        mlua::Error::CallbackError { cause, .. } => first_line(&cause.to_string()),
        other => first_line(&other.to_string()),
    };
    if let Some(host) = challenged {
        return format!(
            "{name} wants a browser check that Raygun can't do. Try again later ({host}: {detail})"
        );
    }
    let lower = detail.to_ascii_lowercase();
    if lower.contains("not enough memory") {
        return format!("{name} sent more than Raygun can hold ({detail})");
    }
    if lower.contains("ran too long") {
        return format!("{name} took too long to answer ({detail})");
    }
    if let Some(library) = detail.strip_prefix("Missing Library:") {
        let library = library.trim();
        return format!(
            "{name} needs the library {library}, which isn't installed. Install the source again"
        );
    }
    if UNSUPPORTED.iter().any(|marker| lower.contains(marker)) {
        return format!("Raygun can't run part of {name} yet ({detail})");
    }
    if PAGE_CHANGED.iter().any(|marker| lower.contains(marker)) {
        return format!("{name} changed its site, so Raygun can't read this page ({detail})");
    }
    // The script's own words (`error("…")`), meant for its readers.
    format!("{name}: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_and_tracebacks_are_dropped() {
        assert_eq!(
            first_line(
                "runtime error: [string \"ext(shosetsu.en.x)\"]:617: attempt to index local 'htmlElement' (a nil value)\nstack traceback:\n\t[C]: in ?"
            ),
            "attempt to index local 'htmlElement' (a nil value)"
        );
        assert_eq!(
            first_line("[string \"prelude\"]:157: [string \"lib(x)\"]:3: boom"),
            "boom"
        );
        assert_eq!(first_line("plain message"), "plain message");
        assert_eq!(
            first_line("[string \"x\"]: not a position"),
            "[string \"x\"]: not a position"
        );
    }

    #[test]
    // mlua's own errors hold their causes this way.
    #[allow(clippy::arc_with_non_send_sync)]
    fn failures_read_as_what_happened() {
        let lua_error = |text: &str| mlua::Error::runtime(text.to_owned());
        let wrapped = |fetch: FetchError| mlua::Error::CallbackError {
            traceback: String::new(),
            cause: std::sync::Arc::new(mlua::Error::external(fetch)),
        };
        let check = wrapped(FetchError::BrowserCheck {
            host: "www.royalroad.com".into(),
            status: 403,
        });
        assert!(
            describe("Royal Road", &check, None).starts_with("Royal Road wants a browser check")
        );
        let gone = wrapped(FetchError::Unreachable {
            host: "novelnb.com".into(),
            detail: "No such host is known. (os error 11001)".into(),
        });
        assert!(describe("NovelNB", &gone, None).starts_with("NovelNB's site can't be found"));
        let slow = wrapped(FetchError::Timeout {
            host: "x.org".into(),
        });
        assert!(describe("X", &slow, None).starts_with("X didn't answer in time"));
        let changed = lua_error(
            "[string \"ext(x)\"]:12: attempt to index a nil value\nstack traceback:\n\tx",
        );
        let message = describe("Wattpad", &changed, None);
        assert_eq!(
            message,
            "Wattpad changed its site, so Raygun can't read this page (attempt to index a nil value)"
        );
        // A bot check seen during the call explains a failure on its page.
        assert!(
            describe("Foxaholic", &changed, Some("www.foxaholic.com"))
                .starts_with("Foxaholic wants a browser check")
        );
        assert_eq!(
            describe(
                "NOVA",
                &lua_error("[string \"ext(nova)\"]:110: CAPTCHA detectado."),
                None
            ),
            "NOVA: CAPTCHA detectado."
        );
        assert!(
            describe(
                "Kemono",
                &mlua::Error::MemoryError("not enough memory".into()),
                None
            )
            .contains("more than Raygun can hold")
        );
        assert!(
            describe("Y", &lua_error("Missing Library: Madara"), None)
                .contains("needs the library Madara")
        );
        assert!(
            describe("Z", &lua_error("unsupported pseudo-class :weird"), None)
                .starts_with("Raygun can't run part of Z")
        );
    }
}
