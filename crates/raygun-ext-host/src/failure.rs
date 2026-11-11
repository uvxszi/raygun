//! Why a provider call failed, in words a listener understands: what the script threw, read
//! together with what its requests met (a browser check, a 429, a lookup that failed), so the
//! message says what happened rather than what the script made of it.

use raygun_net::issue::{self, Issue, IssueKind};

use crate::browser_check::CheckedRequest;
use crate::{CallError, Exhaustion};

/// Something a call's requests met: a failed request or a failed answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trouble {
    pub kind: IssueKind,
    /// "comics.example answered HTTP 403 with a browser check".
    pub detail: String,
}

/// A failed call as its runtime saw it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Failure {
    /// What the script threw, or the runtime's own reason.
    pub message: String,
    /// The worst thing the call's requests met, if any.
    pub trouble: Option<Trouble>,
    /// Whether a request of the call got a good answer from the site: a script error after that
    /// means the answer wasn't in the shape the extension reads.
    pub answered: bool,
    /// The request that met a browser check, as the source sent it (`crate::browser_check`).
    pub checked: Option<Box<CheckedRequest>>,
}

impl Failure {
    /// A failure known only by its message (Shosetsu scripts, the runtime's own errors).
    pub fn said(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Self::default()
        }
    }
}

/// What `raygun.needsSetup(reason)` throws (and a server whose sign-in was refused fails with):
/// the source says it can't work until someone sets it up. However a script wraps the error, the
/// mark stays in its message.
pub(crate) const NEEDS_SETUP_MARK: &str = "[raygun:needs-setup]";

/// What's said after the mark, else a general reason.
fn setup_reason(message: &str) -> String {
    let after = message
        .split_once(NEEDS_SETUP_MARK)
        .map_or("", |(_, rest)| rest)
        .lines()
        .next()
        .unwrap_or_default()
        .trim();
    if after.is_empty() {
        "it said its key or sign-in is missing or was refused".to_owned()
    } else {
        after.to_owned()
    }
}

/// The error a failed call of `provider`'s `method` gives, `name` being the source's name.
pub fn into_error(failure: Failure, name: &str, provider: &str, method: &str) -> CallError {
    let Failure {
        message,
        trouble,
        answered,
        ..
    } = failure;
    let message = message.trim().to_owned();
    if message.contains(NEEDS_SETUP_MARK) {
        return CallError::NeedsSetup {
            name: name.to_owned(),
            extension: provider
                .split_once('/')
                .map_or(provider, |(extension, _)| extension)
                .to_owned(),
            reason: setup_reason(&message),
        };
    }
    if message.contains("has no method") || message.contains("isn't a novel source call") {
        return CallError::MissingMethod {
            name: name.to_owned(),
            provider: provider.to_owned(),
            method: method.to_owned(),
        };
    }
    if let Some(what) = exhaustion(&message) {
        return CallError::Exhausted {
            name: name.to_owned(),
            what,
            detail: message,
        };
    }
    let said = issue::text_kind(&message);
    if let Some(trouble) = trouble {
        // The requests' trouble explains the failure unless the script said something more
        // specific: a plain sentence beats a 404 it got along the way, never a browser check.
        let wins = match said {
            None => trouble.kind.weight() >= IssueKind::Timeout.weight(),
            Some(IssueKind::ExtensionFailed) => true,
            Some(kind) => trouble.kind.weight() >= kind.weight(),
        };
        if wins {
            return failed(trouble.kind, name, &trouble.detail, &message);
        }
    }
    match said {
        Some(IssueKind::ExtensionFailed) if answered => {
            failed(IssueKind::SiteChanged, name, "", &message)
        }
        Some(kind) => failed(kind, name, "", &message),
        None if message.is_empty() => {
            failed(IssueKind::ExtensionFailed, name, "", "no reason given")
        }
        None => CallError::Said {
            name: name.to_owned(),
            message,
        },
    }
}

/// What a script ran out of, by what its runtime threw: QuickJS's "out of memory", "Maximum call
/// stack size exceeded" (its regular expressions' "stack overflow"), or "interrupted" when it
/// ran past its time without waiting.
pub(crate) fn exhaustion(message: &str) -> Option<Exhaustion> {
    if message.contains("out of memory") {
        Some(Exhaustion::Memory)
    } else if message.contains("Maximum call stack size exceeded")
        || message.contains("stack overflow")
    {
        Some(Exhaustion::Stack)
    } else if message.contains("interrupted") {
        Some(Exhaustion::Time)
    } else {
        None
    }
}

fn failed(kind: IssueKind, name: &str, cause: &str, message: &str) -> CallError {
    let detail = [cause.trim(), message.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    CallError::Failed(Issue::new(kind, name, detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(failure: Failure) -> CallError {
        into_error(
            failure,
            "Example Comics",
            "dev.example.comics/comics",
            "search",
        )
    }

    fn kind(failure: Failure) -> Option<IssueKind> {
        match error(failure) {
            CallError::Failed(issue) => Some(issue.kind),
            _ => None,
        }
    }

    #[test]
    fn a_browser_check_explains_whatever_the_script_made_of_it() {
        let checked = Failure {
            message: "TypeError: cannot read property 'data' of undefined".into(),
            trouble: Some(Trouble {
                kind: IssueKind::BotCheck,
                detail: "comics.example answered HTTP 403 with a browser check".into(),
            }),
            answered: false,
            checked: None,
        };
        let error = error(checked);
        assert_eq!(
            error.to_string(),
            "Example Comics wants a browser check, which Raygun can't do\ncomics.example answered HTTP 403 with a browser check\nTypeError: cannot read property 'data' of undefined"
        );
        // Even when the script reported the status it saw.
        assert_eq!(
            kind(Failure {
                message: "Example Comics answered HTTP 403".into(),
                trouble: Some(Trouble {
                    kind: IssueKind::BotCheck,
                    detail: "x".into()
                }),
                answered: false,
                checked: None,
            }),
            Some(IssueKind::BotCheck)
        );
    }

    #[test]
    fn script_errors_after_an_answer_mean_the_site_changed() {
        let message = "TypeError: cannot read property 'title' of undefined";
        assert_eq!(
            kind(Failure {
                message: message.into(),
                trouble: None,
                answered: true,
                checked: None,
            }),
            Some(IssueKind::SiteChanged)
        );
        assert_eq!(
            kind(Failure::said(message)),
            Some(IssueKind::ExtensionFailed)
        );
    }

    #[test]
    fn running_out_says_what_ran_out_with_the_runtime_s_account() {
        let detail = "InternalError: out of memory\nits memory reached 96 MB, its limit";
        let memory = error(Failure::said(detail));
        assert_eq!(
            memory,
            CallError::Exhausted {
                name: "Example Comics".into(),
                what: Exhaustion::Memory,
                detail: detail.into(),
            }
        );
        assert_eq!(
            memory.to_string(),
            format!("Example Comics needed more memory than a source may use\n{detail}")
        );
        assert_eq!(memory.kind(), Some(IssueKind::ExtensionFailed));
        assert_eq!(memory.detail(), detail);
        // Even when a request on the way met trouble: running out is what stopped it.
        let interrupted = error(Failure {
            message: "InternalError: interrupted".into(),
            trouble: Some(Trouble {
                kind: IssueKind::NotFound,
                detail: "x answered HTTP 404".into(),
            }),
            answered: true,
            checked: None,
        });
        assert!(matches!(
            interrupted,
            CallError::Exhausted {
                what: Exhaustion::Time,
                ..
            }
        ));
        assert!(
            interrupted
                .to_string()
                .starts_with("Example Comics kept the processor busy too long"),
            "{interrupted}"
        );
        assert!(matches!(
            error(Failure::said(
                "RangeError: Maximum call stack size exceeded"
            )),
            CallError::Exhausted {
                what: Exhaustion::Stack,
                ..
            }
        ));
    }

    #[test]
    fn statuses_the_script_names_are_read() {
        assert_eq!(
            kind(Failure::said("Example Comics answered HTTP 503")),
            Some(IssueKind::ServerError)
        );
        assert_eq!(
            kind(Failure::said(
                "HTTP 429 from https://api.comics.example/manga"
            )),
            Some(IssueKind::RateLimited)
        );
        assert_eq!(
            error(Failure::said("Example Comics answered HTTP 404")).to_string(),
            "Not found on Example Comics\nExample Comics answered HTTP 404"
        );
    }

    #[test]
    fn a_source_saying_it_needs_setup_is_told_apart_from_failures() {
        // However the script wrapped it, and whatever its requests met.
        let said = error(Failure {
            message: format!("Search failed: Error: {NEEDS_SETUP_MARK} the API key was refused"),
            trouble: Some(Trouble {
                kind: IssueKind::Denied,
                detail: "api.comics.example answered HTTP 401".into(),
            }),
            answered: false,
            checked: None,
        });
        assert_eq!(
            said,
            CallError::NeedsSetup {
                name: "Example Comics".into(),
                extension: "dev.example.comics".into(),
                reason: "the API key was refused".into(),
            }
        );
        assert!(said.is_needs_setup());
        assert_eq!(said.kind(), None, "not a failure of any kind");
        assert_eq!(
            said.to_string(),
            "Example Comics needs a key or sign-in\nneeds setup (dev.example.comics): the API key was refused"
        );
        assert_eq!(
            crate::host::needs_setup_in(&said.to_string()),
            Some("dev.example.comics")
        );
        assert_eq!(
            crate::host::needs_setup_in("Example Comics answered HTTP 401"),
            None
        );
        // Without a reason, a general one.
        assert!(matches!(
            error(Failure::said(NEEDS_SETUP_MARK)),
            CallError::NeedsSetup { reason, .. } if reason.contains("key or sign-in")
        ));
    }

    #[test]
    fn plain_sentences_are_kept_and_missing_calls_told_apart() {
        assert_eq!(
            error(Failure::said("This chapter has no pages")),
            CallError::Said {
                name: "Example Comics".into(),
                message: "This chapter has no pages".into(),
            }
        );
        // A small trouble along the way doesn't beat the script's own words.
        assert!(matches!(
            error(Failure {
                message: "This book has no catalogue entry".into(),
                trouble: Some(Trouble {
                    kind: IssueKind::NotFound,
                    detail: "x answered HTTP 404".into()
                }),
                answered: true,
                checked: None,
            }),
            CallError::Said { .. }
        ));
        // A network failure does.
        assert_eq!(
            kind(Failure {
                message: "Search failed".into(),
                trouble: Some(Trouble {
                    kind: IssueKind::Dns,
                    detail: "could not reach x: dns error".into()
                }),
                answered: false,
                checked: None,
            }),
            Some(IssueKind::Dns)
        );
        assert!(matches!(
            error(Failure::said("provider comics has no method filters")),
            CallError::MissingMethod { .. }
        ));
    }
}
