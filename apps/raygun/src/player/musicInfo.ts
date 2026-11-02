import { commands } from '@raygun/bindings';
import type { ArtistInfo, CreditGroup, QueueItem, SongCredits, TrackQuery } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import { creditedArtists } from '../music/artists';

/** What open databases say about an artist (M46.14), or `null` when no source knows them. */
export const useArtistInfo = (name: string | undefined) =>
  useQuery<ArtistInfo | null>({
    queryKey: ['music', 'artistInfo', name?.toLowerCase()],
    queryFn: () => unwrapResult(commands.musicArtistInfo(name ?? '')),
    enabled: Boolean(name),
    staleTime: Infinity,
    retry: 1,
  });

/** Who made a song (M46.14), or `null` when no source knows it. */
export const useSongCredits = (query: TrackQuery) =>
  useQuery<SongCredits | null>({
    queryKey: ['music', 'credits', query.title, query.artist, query.album],
    queryFn: () => unwrapResult(commands.musicSongCredits(query)),
    staleTime: Infinity,
    retry: 1,
  });

/** The credits to show: the source's, else the item's own artists as its performers. */
export const shownCredits = (
  credits: SongCredits | null | undefined,
  item: QueueItem,
  performedBy: string,
): CreditGroup[] => {
  if (credits && credits.credits.length > 0) return credits.credits;
  const names = creditedArtists(item.artist);
  return names.length > 0 ? [{ role: performedBy, names }] : [];
};

/** "Detroit · 1954–": where an artist is from and when they were active. */
export const artistFacts = (info: ArtistInfo | null | undefined) =>
  [info?.origin, info?.active].filter(Boolean).join(' · ');
