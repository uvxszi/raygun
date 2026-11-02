import { commands, events } from '@raygun/bindings';
import type { MixKind, MixSummary } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { TFunction } from 'i18next';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const FOR_YOU = ['music', 'forYou'] as const;

/** Minutes to add to UTC for this device's clock (mixes follow the user's day). */
const utcOffset = () => -new Date().getTimezoneOffset();

/** Made for You (M46.20): today's mixes and listening numbers; Rust makes them in the background. */
export const useMadeForYou = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.madeForYouEvent.listen(() => {
      void client.invalidateQueries({ queryKey: FOR_YOU });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  return useQuery({
    queryKey: [...FOR_YOU, 'overview'],
    queryFn: () => unwrapResult(commands.madeForYou(false, utcOffset())),
    staleTime: 5 * 60_000,
  });
};

/** Makes the mixes again now. */
export const useRefreshMadeForYou = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.madeForYou(true, utcOffset())),
    onSuccess: (overview) => client.setQueryData([...FOR_YOU, 'overview'], overview),
  });
};

/** One mix with its songs. */
export const useMix = (id: string) =>
  useQuery({
    queryKey: [...FOR_YOU, 'mix', id],
    queryFn: () => unwrapResult(commands.madeForYouMix(id)),
    enabled: id !== '',
  });

/** Saves a mix as a playlist. */
export const useSaveMix = () =>
  useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      unwrapResult(commands.madeForYouSave(id, name)),
  });

type MixText = Pick<MixSummary, 'kind' | 'seed' | 'number' | 'artists'>;

/** A mix's name, in the user's language ("Daily Mix 2", "Because you listened to Nina Simone"). */
export const mixTitle = (t: TFunction, mix: MixText) => {
  const seed = mix.seed ?? '';
  switch (mix.kind) {
    case 'daily':
      return t('forYou.titles.daily', { number: mix.number ?? 1 });
    case 'timeOfDay':
      return t(`forYou.titles.time.${seed || 'evening'}`);
    case 'becauseYouListened':
      return t('forYou.titles.because', { artist: seed });
    case 'artistRadio':
      return t('forYou.titles.radio', { artist: seed });
    case 'genre':
    case 'mood':
      return t('forYou.titles.themed', { name: seed });
    default:
      return t(`forYou.titles.${mix.kind}`);
  }
};

/** What a mix is: the artists in it, or what it's for. */
export const mixDescription = (t: TFunction, mix: MixText) => {
  const [first, second, third] = mix.artists;
  const artists = third
    ? t('forYou.artists3', { first, second, third })
    : second
      ? t('forYou.artists2', { first, second })
      : (first ?? '');
  // Daily Mixes say only who's in them.
  const about = mix.kind === 'daily' ? '' : t(`forYou.about.${mix.kind}`);
  return [about, artists].filter(Boolean).join(' · ');
};

/** The order Made for You shows kinds of mix in, by shelf. */
export const SHELVES: { id: string; kinds: MixKind[] }[] = [
  { id: 'daily', kinds: ['daily'] },
  {
    id: 'made',
    kinds: ['discoverWeekly', 'releaseRadar', 'onRepeat', 'timeOfDay', 'repeatRewind'],
  },
  { id: 'because', kinds: ['becauseYouListened', 'artistRadio'] },
  { id: 'themed', kinds: ['genre', 'mood'] },
];
