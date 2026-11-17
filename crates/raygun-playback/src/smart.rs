//! Smart Shuffle (a Smart Chip): recommended songs mixed into a shuffled list, about one for
//! every [`EVERY`] of the list's own. The core keeps them apart from the list's songs: they're
//! marked, leave when Smart Shuffle or shuffle goes off, and make way for new ones when it's turned
//! on again. Which songs they are is the service's business.

use raygun_model::{QueueItem, QueueItemInput};

use crate::{Effect, PlaybackCore};

/// One suggestion for every this many of the list's own songs.
pub const EVERY: usize = 3;

/// How many suggestions fit among `own` upcoming songs: one after each [`EVERY`], and one after
/// a shorter list.
pub fn room_for(own: usize) -> usize {
    match own {
        0 => 0,
        own => (own / EVERY).max(1),
    }
}

/// `own` in its order with `added` placed after each [`EVERY`] of them (after the last when the
/// list is shorter). Only [`room_for`] of `added` are placed; the rest are left out.
pub fn mix_in<T: Copy>(own: &[T], added: &[T]) -> Vec<T> {
    let room = room_for(own.len()).min(added.len());
    let mut mixed = Vec::with_capacity(own.len() + room);
    let mut added = added.iter().take(room).copied();
    for (index, item) in own.iter().copied().enumerate() {
        mixed.push(item);
        let placed = index + 1;
        // After each block of `EVERY`; a shorter list's one at its end.
        let block_end = placed % EVERY == 0;
        let short_end = own.len() < EVERY && placed == own.len();
        if block_end || short_end {
            mixed.extend(added.next());
        }
    }
    mixed
}

impl PlaybackCore {
    pub fn smart_shuffle(&self) -> bool {
        self.smart_shuffle && self.shuffle
    }

    /// Turns Smart Shuffle on (shuffle too) or off. Either way the songs it added before go (the
    /// one playing finishes): on asks the service for new ones ([`Self::add_suggestions`]).
    pub fn set_smart_shuffle(&mut self, enabled: bool) -> Vec<Effect> {
        let mut effects = Vec::new();
        if enabled && !self.shuffle {
            effects.extend(self.set_shuffle(true));
        }
        self.smart_shuffle = enabled;
        if self.drop_suggested() {
            effects.push(Effect::QueueChanged);
        }
        effects.push(Effect::StateChanged);
        effects
    }

    /// How many suggestions the songs still to come have room for.
    pub fn suggestion_room(&self) -> usize {
        room_for(self.upcoming_own().len())
    }

    /// The list's own songs (not Smart Shuffle's), in queue order.
    pub fn own_items(&self) -> Vec<&QueueItem> {
        self.items
            .iter()
            .filter(|item| !self.suggested.contains(&item.id))
            .collect()
    }

    pub fn is_suggested(&self, item_id: &str) -> bool {
        self.suggested.contains(item_id)
    }

    /// Mixes `inputs` into the songs to come, in place of the suggestions not played yet. Nothing
    /// changes while Smart Shuffle is off (an answer that came too late).
    pub fn add_suggestions(&mut self, inputs: Vec<QueueItemInput>) -> Vec<Effect> {
        if !self.smart_shuffle() {
            return Vec::new();
        }
        self.drop_suggested();
        let upcoming = self.upcoming_own();
        let room = room_for(upcoming.len()).min(inputs.len());
        let mut added = Vec::with_capacity(room);
        for input in inputs.into_iter().take(room) {
            let index = self.items.len();
            let item = item_of(input);
            self.suggested.insert(item.id.clone());
            self.items.push(item);
            added.push(index);
        }
        let start = self.cursor.map_or(0, |cursor| cursor + 1);
        self.order.truncate(start);
        self.order.extend(mix_in(&upcoming, &added));
        vec![Effect::QueueChanged]
    }

    /// A suggested song becomes one of the list's own (added to the playlist it came from).
    pub fn keep_suggested(&mut self, item_id: &str) -> Vec<Effect> {
        if self.suggested.remove(item_id) {
            vec![Effect::QueueChanged]
        } else {
            Vec::new()
        }
    }

    /// Marks restored items (by index) as Smart Shuffle's, and puts its switch back.
    pub fn mark_suggested(&mut self, indexes: &[usize], smart_shuffle: bool) -> Vec<Effect> {
        self.smart_shuffle = smart_shuffle && self.shuffle;
        for &index in indexes {
            if let Some(item) = self.items.get(index) {
                self.suggested.insert(item.id.clone());
            }
        }
        vec![Effect::QueueChanged, Effect::StateChanged]
    }

    /// Indexes of the suggested items in queue order (what a moved or saved player carries).
    pub fn suggested_indexes(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.suggested.contains(&item.id))
            .map(|(index, _)| index)
            .collect()
    }

    /// The list's own songs after the current one, in play order (indexes into `items`).
    fn upcoming_own(&self) -> Vec<usize> {
        let start = self.cursor.map_or(0, |cursor| cursor + 1);
        self.order
            .iter()
            .skip(start)
            .copied()
            .filter(|&index| !self.suggested.contains(&self.items[index].id))
            .collect()
    }

    /// Removes the suggested items except the one playing; whether any went.
    pub(crate) fn drop_suggested(&mut self) -> bool {
        let current = self.cursor.map(|cursor| self.order[cursor]);
        let keep: Vec<bool> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| Some(index) == current || !self.suggested.contains(&item.id))
            .collect();
        if keep.iter().all(|kept| *kept) {
            return false;
        }
        let mut moved_to = vec![usize::MAX; keep.len()];
        let mut items = Vec::with_capacity(self.items.len());
        for (index, item) in std::mem::take(&mut self.items).into_iter().enumerate() {
            if keep[index] {
                moved_to[index] = items.len();
                items.push(item);
            } else {
                self.suggested.remove(&item.id);
            }
        }
        self.items = items;
        self.order = self
            .order
            .iter()
            .filter(|&&index| keep[index])
            .map(|&index| moved_to[index])
            .collect();
        self.cursor = current.map(|current| self.position_in_order(moved_to[current]));
        true
    }
}

fn item_of(input: QueueItemInput) -> QueueItem {
    QueueItem {
        explicit: input.explicit,
        id: uuid::Uuid::now_v7().to_string(),
        title: input.title,
        artist: input.artist,
        album: input.album,
        duration_ms: input.duration_ms,
        artwork_url: input.artwork_url,
        media_kind: input.media_kind,
        source: input.source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_suggestion_follows_every_three_songs() {
        let own = [1, 2, 3, 4, 5, 6, 7];
        assert_eq!(mix_in(&own, &[10, 11, 12]), [1, 2, 3, 10, 4, 5, 6, 11, 7]);
        // Fewer suggestions than room: what there is, in its place.
        assert_eq!(mix_in(&own, &[10]), [1, 2, 3, 10, 4, 5, 6, 7]);
        // A short list gets one, at its end.
        assert_eq!(mix_in(&[1, 2], &[10, 11]), [1, 2, 10]);
        assert_eq!(mix_in(&[1], &[10]), [1, 10]);
        assert_eq!(mix_in::<i32>(&[], &[10]), Vec::<i32>::new());
        assert_eq!(room_for(0), 0);
        assert_eq!(room_for(2), 1);
        assert_eq!(room_for(6), 2);
        assert_eq!(room_for(10), 3);
    }
}
