//! What extensions are doing now, for crash records: the app hears each extension's load and call
//! begin and end ([`set_hook`]), so a crash record (or the next start, after a crash that left no
//! record) can name the sources that were running. An activity ends when its guard drops, so a
//! call stopped halfway ends too; only a crash leaves one begun and never ended.

use std::sync::{Arc, PoisonError, RwLock};

/// One step of an extension's work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity<'a> {
    /// Its code is being loaded (`id`).
    Loading(&'a str),
    /// One of its providers' methods runs: the extension's id and `provider.method`.
    Calling(&'a str, &'a str),
}

/// A begun activity, or one that ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Began,
    Ended,
}

type Hook = Arc<dyn Fn(Activity<'_>, Step) + Send + Sync>;

static HOOK: RwLock<Option<Hook>> = RwLock::new(None);

/// Installs what hears every activity (the app's crash records). Replaces the one before.
pub fn set_hook(hook: impl Fn(Activity<'_>, Step) + Send + Sync + 'static) {
    *HOOK.write().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(hook));
}

fn tell(activity: Activity<'_>, step: Step) {
    let hook = HOOK.read().unwrap_or_else(PoisonError::into_inner).clone();
    if let Some(hook) = hook {
        hook(activity, step);
    }
}

/// Tells the hook an activity began, and that it ended once dropped.
pub(crate) struct Noted {
    extension: String,
    call: Option<String>,
}

impl Noted {
    pub(crate) fn loading(extension: &str) -> Self {
        tell(Activity::Loading(extension), Step::Began);
        Self {
            extension: extension.to_owned(),
            call: None,
        }
    }

    pub(crate) fn calling(extension: &str, call: &str) -> Self {
        tell(Activity::Calling(extension, call), Step::Began);
        Self {
            extension: extension.to_owned(),
            call: Some(call.to_owned()),
        }
    }
}

impl Drop for Noted {
    fn drop(&mut self) {
        let activity = match &self.call {
            Some(call) => Activity::Calling(&self.extension, call),
            None => Activity::Loading(&self.extension),
        };
        tell(activity, Step::Ended);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn activities_begin_and_end_with_their_guard() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let log = heard.clone();
        set_hook(move |activity, step| {
            let what = match activity {
                Activity::Loading(id) => format!("load {id}"),
                Activity::Calling(id, call) => format!("call {id} {call}"),
            };
            log.lock().unwrap().push(format!("{what} {step:?}"));
        });
        {
            let _load = Noted::loading("activity.test");
            let _call = Noted::calling("activity.test", "music.search");
        }
        let heard = heard.lock().unwrap().clone();
        let ours: Vec<_> = heard
            .into_iter()
            .filter(|line| line.contains("activity.test"))
            .collect();
        assert_eq!(
            ours,
            [
                "load activity.test Began",
                "call activity.test music.search Began",
                "call activity.test music.search Ended",
                "load activity.test Ended",
            ]
        );
    }
}
