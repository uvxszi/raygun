//! Streams found ahead (M46.53): while a song plays, the songs Next and Previous would load (the
//! ones around it in the queue) have their streams found, so pressing Next or Previous, or the
//! song ending, plays at once. A stream found ahead plays only for the queue item it was found
//! for, under the same names, while it still works, found on the same kind of network (the
//! quality aimed for follows it). One about to expire is found again before its song is due.
//! Pure bookkeeping; `PlaybackService` runs the searches, and stops those no longer wanted.

use std::collections::HashMap;
use std::time::Duration;

use raygun_resolver::Pick;

/// Searches start this long after a song starts playing: its own stream comes first.
pub const AFTER: Duration = Duration::from_secs(4);
/// A stream found ahead is kept unused this long (ms).
const KEPT_FOR_MS: f64 = 30.0 * 60.0 * 1_000.0;
/// A stream expiring within this long (ms) is found again; one that close isn't used.
const FRESH_FOR_MS: i64 = 2 * 60 * 1_000;
/// A song nothing was found for is looked for again after this long (ms), if still wanted.
const MISSED_FOR_MS: f64 = 5.0 * 60.0 * 1_000.0;

/// A queue item whose stream is wanted ahead, and the resolver's key of the names it's found by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub item_id: String,
    pub key: String,
}

/// What to do now: the searches to start, and those to stop (by queue item id).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub search: Vec<Wanted>,
    pub stop: Vec<String>,
}

#[derive(Debug, Clone)]
struct Found {
    key: String,
    pick: Pick,
    /// When it was found, on the service's clock (ms).
    at_ms: f64,
    /// It was found over mobile data.
    mobile: bool,
}

impl Found {
    /// Whether it can play for `key` now: the same names, not kept too long, found on the same
    /// kind of network, and its stream working for a while yet.
    fn plays(&self, key: &str, now_ms: f64, unix_ms: i64, mobile: Option<bool>) -> bool {
        self.key == key
            && now_ms - self.at_ms < KEPT_FOR_MS
            && mobile.is_none_or(|mobile| mobile == self.mobile)
            && self
                .pick
                .expires_at
                .is_none_or(|expires| expires - FRESH_FOR_MS > unix_ms)
    }
}

/// The streams found ahead, and the searches finding them.
#[derive(Debug, Default)]
pub struct Ahead {
    /// Streams found, by queue item id.
    found: HashMap<String, Found>,
    /// Searches running, by queue item id: the key each searches for.
    running: HashMap<String, String>,
    /// Songs a search found nothing for lately, by queue item id: the key, and when.
    missed: HashMap<String, (String, f64)>,
}

impl Ahead {
    /// The songs around the current one are `wanted` now (`mobile`: over mobile data, `None` when
    /// unknown). What's no longer wanted goes, and its search stops; what's wanted and has no
    /// stream that plays (none found yet, found under other names or on another network, about
    /// to expire) is searched for, unless a search for it runs or found nothing lately.
    pub fn want(
        &mut self,
        wanted: &[Wanted],
        now_ms: f64,
        unix_ms: i64,
        mobile: Option<bool>,
    ) -> Plan {
        let is_wanted = |item_id: &str, key: &str| {
            wanted
                .iter()
                .any(|want| want.item_id == item_id && want.key == key)
        };
        let mut plan = Plan::default();
        self.found.retain(|item_id, found| {
            is_wanted(item_id, &found.key) && found.plays(&found.key, now_ms, unix_ms, mobile)
        });
        self.missed
            .retain(|item_id, (key, at)| is_wanted(item_id, key) && now_ms - *at < MISSED_FOR_MS);
        let stopped: Vec<String> = self
            .running
            .iter()
            .filter(|(item_id, key)| !is_wanted(item_id, key))
            .map(|(item_id, _)| item_id.clone())
            .collect();
        for item_id in stopped {
            self.running.remove(&item_id);
            plan.stop.push(item_id);
        }
        for want in wanted {
            if self.found.contains_key(&want.item_id)
                || self.running.contains_key(&want.item_id)
                || self.missed.contains_key(&want.item_id)
            {
                continue;
            }
            self.running.insert(want.item_id.clone(), want.key.clone());
            plan.search.push(want.clone());
        }
        plan
    }

    /// A search for `item_id` under `key` finished with `pick` (or nothing). Kept when that song
    /// is still wanted under those names; whether it was.
    pub fn found(
        &mut self,
        item_id: &str,
        key: &str,
        pick: Option<Pick>,
        now_ms: f64,
        mobile: bool,
    ) -> bool {
        if self
            .running
            .get(item_id)
            .is_none_or(|running| running != key)
        {
            return false;
        }
        self.running.remove(item_id);
        match pick {
            Some(pick) => {
                self.found.insert(
                    item_id.to_owned(),
                    Found {
                        key: key.to_owned(),
                        pick,
                        at_ms: now_ms,
                        mobile,
                    },
                );
            }
            None => {
                self.missed
                    .insert(item_id.to_owned(), (key.to_owned(), now_ms));
            }
        }
        true
    }

    /// The stream found ahead for `item_id` under `key`, when it plays now (see [`Found::plays`]).
    /// Taken: it's the load's now.
    pub fn take(
        &mut self,
        item_id: &str,
        key: &str,
        now_ms: f64,
        unix_ms: i64,
        mobile: Option<bool>,
    ) -> Option<Pick> {
        let found = self.found.remove(item_id)?;
        found
            .plays(key, now_ms, unix_ms, mobile)
            .then_some(found.pick)
    }

    /// Whether the stream found for `item_id` stops working before `unix_ms` (and some): the
    /// song due then needs it found again.
    pub fn expires_before(&self, item_id: &str, unix_ms: i64) -> bool {
        self.found.get(item_id).is_some_and(|found| {
            found
                .pick
                .expires_at
                .is_some_and(|expires| expires - FRESH_FOR_MS <= unix_ms)
        })
    }

    /// Forgets the stream found for `item_id`, so the next [`Self::want`] finds it again.
    pub fn forget(&mut self, item_id: &str) {
        self.found.remove(item_id);
    }

    /// Nothing is wanted anymore (the queue emptied): returns the searches to stop.
    pub fn clear(&mut self) -> Vec<String> {
        self.found.clear();
        self.missed.clear();
        self.running.drain().map(|(item_id, _)| item_id).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use raygun_model::MediaSource;
    use raygun_resolver::CandidateKey;

    use super::*;

    fn want(item_id: &str, key: &str) -> Wanted {
        Wanted {
            item_id: item_id.into(),
            key: key.into(),
        }
    }

    fn pick(id: &str, expires_at: Option<i64>) -> Pick {
        Pick {
            key: CandidateKey {
                provider_id: "dev.example/stream".into(),
                candidate_id: id.into(),
            },
            track_key: "band|song".into(),
            title: "Song".into(),
            artist: None,
            provider_name: "Example".into(),
            source: MediaSource::Url {
                url: format!("https://example.org/{id}.mp3"),
                headers: BTreeMap::new(),
            },
            expires_at,
            from_cache: false,
            bitrate_kbps: None,
        }
    }

    const NOW: i64 = 1_000_000_000;

    #[test]
    fn the_songs_around_the_one_playing_are_searched_for_once() {
        let mut ahead = Ahead::default();
        let wanted = [want("next", "band|next"), want("before", "band|before")];
        let plan = ahead.want(&wanted, 0.0, NOW, Some(false));
        assert_eq!(plan.search, wanted);
        assert!(plan.stop.is_empty());
        // Asked again while the searches run: nothing new.
        assert_eq!(ahead.want(&wanted, 10.0, NOW, Some(false)), Plan::default());

        // Found: the next song plays it at once, once.
        assert!(ahead.found("next", "band|next", Some(pick("a", None)), 20.0, false));
        assert_eq!(
            ahead.take("next", "band|next", 30.0, NOW, Some(false)),
            Some(pick("a", None))
        );
        assert_eq!(
            ahead.take("next", "band|next", 30.0, NOW, Some(false)),
            None
        );
        // Nothing found for the song before: not searched again for a while.
        assert!(ahead.found("before", "band|before", None, 20.0, false));
        assert!(
            ahead
                .want(&wanted[1..], 40.0, NOW, Some(false))
                .search
                .is_empty()
        );
        assert_eq!(
            ahead
                .want(&wanted[1..], 40.0 + MISSED_FOR_MS, NOW, Some(false))
                .search,
            wanted[1..]
        );
    }

    #[test]
    fn a_queue_that_changed_stops_what_isnt_wanted_anymore() {
        let mut ahead = Ahead::default();
        ahead.want(&[want("b", "x|b"), want("c", "x|c")], 0.0, NOW, None);
        assert!(ahead.found("b", "x|b", Some(pick("b", None)), 1.0, false));
        // The queue moved on: c is gone, its search stops; b is still wanted and kept; d is new.
        let plan = ahead.want(&[want("b", "x|b"), want("d", "x|d")], 2.0, NOW, None);
        assert_eq!(plan.stop, ["c"]);
        assert_eq!(plan.search, [want("d", "x|d")]);
        // What a stopped search finds comes too late.
        assert!(!ahead.found("c", "x|c", Some(pick("c", None)), 3.0, false));
        assert_eq!(ahead.take("c", "x|c", 3.0, NOW, None), None);
        // A song given other names is found again under them.
        let plan = ahead.want(&[want("b", "x|b live"), want("d", "x|d")], 4.0, NOW, None);
        assert_eq!(plan.search, [want("b", "x|b live")]);
        assert!(!ahead.found("b", "x|b", Some(pick("b", None)), 5.0, false));
        assert_eq!(ahead.take("b", "x|b", 5.0, NOW, None), None);
        // The queue emptied: everything goes.
        assert_eq!(
            {
                let mut stopped = ahead.clear();
                stopped.sort();
                stopped
            },
            ["b", "d"]
        );
    }

    #[test]
    fn a_stream_found_ahead_plays_only_while_it_works_on_the_same_network() {
        let mut ahead = Ahead::default();
        let wanted = [want("next", "band|next")];
        ahead.want(&wanted, 0.0, NOW, Some(false));
        let expires = NOW + 10 * 60 * 1_000;
        ahead.found(
            "next",
            "band|next",
            Some(pick("a", Some(expires))),
            0.0,
            false,
        );
        // Under other names, or after moving to mobile data (another quality): not that one.
        assert_eq!(
            ahead.take("next", "band|other", 1.0, NOW, Some(false)),
            None
        );
        ahead.want(&wanted, 2.0, NOW, Some(false));
        ahead.found(
            "next",
            "band|next",
            Some(pick("a", Some(expires))),
            2.0,
            false,
        );
        assert_eq!(ahead.take("next", "band|next", 3.0, NOW, Some(true)), None);

        // About to expire by the time the song is due: found again, with a fresh stream.
        ahead.want(&wanted, 4.0, NOW, Some(false));
        ahead.found(
            "next",
            "band|next",
            Some(pick("a", Some(expires))),
            4.0,
            false,
        );
        assert!(!ahead.expires_before("next", NOW + 60_000));
        assert!(ahead.expires_before("next", expires - 60_000));
        let later = expires - FRESH_FOR_MS + 1;
        assert_eq!(
            ahead.want(&wanted, 5.0, later, Some(false)).search,
            wanted,
            "an expiring stream is found again"
        );
        ahead.found(
            "next",
            "band|next",
            Some(pick("b", Some(later + 3_600_000))),
            6.0,
            false,
        );
        assert_eq!(
            ahead.take("next", "band|next", 7.0, later, Some(false)),
            Some(pick("b", Some(later + 3_600_000)))
        );

        // Kept unused too long: searched again rather than trusted.
        ahead.want(&wanted, 8.0, NOW, None);
        ahead.found("next", "band|next", Some(pick("c", None)), 8.0, false);
        assert_eq!(
            ahead.take("next", "band|next", 8.0 + KEPT_FOR_MS, NOW, None),
            None
        );
    }
}
