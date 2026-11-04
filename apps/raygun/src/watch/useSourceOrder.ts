import { commands } from '@raygun/bindings';
import type { SourceCandidate, SourceRank, StremioStream, VideoLinkView } from '@raygun/bindings';
import { keepPreviousData, useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

/** A server link's label as sessions give it (`video_session::resolve_source`). */
export const serverLinkLabel = (link: VideoLinkView, fallback: string) =>
  link.label ?? (link.quality ? `${link.quality}p` : fallback);

/** An addon stream's label as sessions give it (`StremioService::playable`). */
export const streamLabel = (stream: StremioStream, addonName: string) =>
  [stream.name, stream.description]
    .filter((text): text is string => Boolean(text))
    .map((text) => text.replaceAll('\n', ' '))
    .join(' · ') || addonName;

/**
 * Sources in the order a session tries them for a title (`group`: `title:<id>` or
 * `stremio:<kind>:<id>`), with what played last time marked. Until it answers, the order given.
 */
export const useSourceOrder = (
  group: string,
  title: string | null,
  sources: SourceCandidate[],
): SourceRank[] | null => {
  const order = useQuery({
    queryKey: ['video', 'sourceOrder', group, title, sources],
    queryFn: () => unwrapResult(commands.videoSourceOrder(group, title, sources)),
    enabled: sources.length > 0,
    placeholderData: keepPreviousData,
  });
  const ranks = order.data;
  // A rank for each source given, else the answer is for an older list.
  return ranks && ranks.length === sources.length ? ranks : null;
};

/** `items` in `ranks`' order (its `index`es), or as they are without ranks. */
export const inRankOrder = <Item>(items: Item[], ranks: SourceRank[] | null) =>
  ranks
    ? ranks.flatMap((rank) => {
        const item = items[rank.index];
        return item === undefined ? [] : [{ item, rank }];
      })
    : items.map((item) => ({ item, rank: null as SourceRank | null }));
