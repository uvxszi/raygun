import { commands } from '@raygun/bindings';
import type { SongEdit, SoundPrefs, TrackQuery } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { withEditDefaults, withSoundDefaults } from './soundPrefs';
import type { FullSongEdit, FullSound } from './soundPrefs';

/** How often a slider being dragged is saved, so the sound follows the thumb. */
const LIVE_SAVE_MS = 150;

/**
 * The sound settings (M46.12) with a draft that follows sliders as they move: each change shows
 * at once and reaches the engine a few times a second, so the equalizer can be heard while
 * dragging.
 */
export const useSoundPrefs = () => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const saved = withSoundDefaults(settings.data?.sound);
  const [draft, setDraft] = useState<FullSound | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latest = useRef<FullSound | null>(null);
  const mutate = update.mutate;

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current);
    },
    [],
  );

  const flush = () => {
    timer.current = null;
    const next = latest.current;
    if (!next) return;
    mutate(
      { key: 'sound', value: next },
      // Once the last change is saved, the stored settings are what's shown.
      { onSettled: () => latest.current === next && setDraft(null) },
    );
  };

  const change = (patch: Partial<SoundPrefs>) => {
    const next: FullSound = { ...(latest.current ?? draft ?? saved), ...withPatch(patch) };
    latest.current = next;
    setDraft(next);
    if (!timer.current) timer.current = setTimeout(flush, LIVE_SAVE_MS);
  };

  return {
    sound: draft ?? saved,
    loading: settings.isPending,
    change,
    error: update.error,
  };
};

/** Drops `undefined` so a partial change can't blank a field. */
const withPatch = (patch: Partial<SoundPrefs>) =>
  Object.fromEntries(Object.entries(patch).filter(([, value]) => value !== undefined));

const songEditKey = (query: TrackQuery) => ['songEdit', query.artist ?? '', query.title];

/** A song's own trims, volume and lyrics edits (M46.12). */
export const useSongEdit = (query: TrackQuery) => {
  const client = useQueryClient();
  const edit = useQuery({
    queryKey: songEditKey(query),
    queryFn: async () => withEditDefaults(await commands.songEditGet(query)),
  });
  const save = useMutation({
    mutationFn: async (next: FullSongEdit) =>
      withEditDefaults(await unwrapResult(commands.songEditSet(query, next as SongEdit))),
    onSuccess: (saved) => {
      client.setQueryData(songEditKey(query), saved);
      // Lyrics come with the song's offset and edits.
      void client.invalidateQueries({ queryKey: ['lyrics'] });
    },
  });
  return { edit, save };
};
