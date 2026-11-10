import { commands } from '@raygun/bindings';
import type { ArtistInfo, OnlineTrack } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useSearch } from '@tanstack/react-router';
import { ExternalLink, LoaderCircle, Pencil, Play, Shuffle, UserRound } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SavedTrackRow } from '../collections/SavedTrackRow';
import { playCollection, useFavorites } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { AlbumGrid } from '../library/AlbumGrid';
import { Artwork } from '../library/Artwork';
import { TrackRow } from '../library/TrackRow';
import { playFromLibrary } from '../library/useLibrary';
import { creditedArtists } from '../music/artists';
import { Discography } from '../music/Discography';
import { usePlayContext } from '../music/playContext';
import { songReveal, useRevealIndex } from '../music/reveal';
import { artistFacts, useArtistInfo } from '../player/musicInfo';
import { OnlineTrackRow } from '../search/OnlineTrackRow';
import { playResults } from '../search/useOnlineSearch';
import { Sheet } from '../shell/Sheet';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { useLate } from '../shell/useLate';
import { DetailEditForm } from '../music/DetailEditForm';
import { useDetailEdit } from '../music/useDetailEdit';
import { SourceIssue } from '../shell/SourceIssue';
import { FollowButton } from '../music/FollowButton';
import { ArtistEvents } from '../music/ArtistEvents';

/** Popular songs shown before "Show more". */
const POPULAR_SHOWN = 10;

const onlineName = (track: OnlineTrack) => songReveal(track.title, track.artist);

const sameArtist = (credit: string | null | undefined, name: string) =>
  creditedArtists(credit).some((artist) => artist.toLowerCase() === name.toLowerCase()) ||
  (credit ?? '').toLowerCase() === name.toLowerCase();

const Section: FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <section aria-label={title} className="flex flex-col gap-2">
    <h2 className="text-lg font-bold">{title}</h2>
    {children}
  </section>
);

/**
 * An artist's page (M46.8), reached from any artist name: popular songs from your sources, their
 * story from open databases (M46.14), then what your library and your saved songs have by them.
 */
export const MusicArtistPage: FC = () => {
  const { t } = useTranslation();
  const { name } = useSearch({ from: '/artist' });
  const overview = useQuery({
    queryKey: ['music', 'artist', name],
    queryFn: () => unwrapResult(commands.musicArtist(name)),
    enabled: name !== '',
    staleTime: 10 * 60_000,
  });
  const info = useArtistInfo(name || undefined);
  // The page comes from what was kept at once; a spinner shows only when it has to be asked for.
  const slow = useLate(overview.isPending && name !== '');
  const favorites = useFavorites();
  const [more, setMore] = useState(false);
  const popular = overview.data?.popular ?? [];
  // "Playing from" asked for a popular song past the first few: show them all.
  const revealAt = useRevealIndex(popular, onlineName);
  if (revealAt >= POPULAR_SHOWN && !more) setMore(true);
  const saved = (favorites.data ?? []).filter((track) => sameArtist(track.artist, name));
  // The user's own name and picture for this artist (M46.30).
  const [editing, setEditing] = useState(false);
  const own = useDetailEdit(name ? { kind: 'artist', name } : null);
  const picture =
    own.data?.artworkUrl ??
    info.data?.imageUrl ??
    overview.data?.albums.find((album) => album.artworkUrl)?.artworkUrl ??
    popular.find((track) => track.artworkUrl)?.artworkUrl ??
    null;
  usePlayContext(
    name
      ? {
          kind: 'artist',
          name: own.data?.title ?? name,
          subtitle: null,
          artworkUrl: picture,
          to: '/artist',
          search: { name },
        }
      : null,
  );
  const play = (shuffle: boolean) => {
    if (popular.length > 0) {
      const start = shuffle ? popular[Math.floor(Math.random() * popular.length)] : popular[0];
      void playResults(popular, start as OnlineTrack);
    } else if ((overview.data?.tracks.length ?? 0) > 0) {
      void playFromLibrary({
        selection: { kind: 'artist', artist: name },
        startTrackId: null,
        shuffle,
      });
    }
  };

  return (
    <section data-testid="music-artist-page" className="flex flex-col gap-6">
      <TopBar
        back
        fallback="/browse"
        plainTitle
        title={t('artist.title')}
        actions={
          <>
            <TopBarAction
              label={t('details.editArtist')}
              icon={Pencil}
              onClick={() => setEditing(true)}
            />
          </>
        }
      />
      {editing && (
        <Sheet title={t('details.editArtist')} onClose={() => setEditing(false)}>
          <DetailEditForm
            target={{ kind: 'artist', name }}
            fields={['title']}
            original={{ title: name, artworkUrl: info.data?.imageUrl ?? null }}
            titleLabel={t('details.artistName')}
            onSaved={() => setEditing(false)}
          />
        </Sheet>
      )}
      <header className="flex flex-col items-center gap-4 sm:flex-row sm:items-end sm:gap-6">
        {picture ? (
          <Artwork
            url={picture}
            className="size-36 rounded-full shadow-xl sm:size-44"
            iconSize={56}
          />
        ) : (
          <span className="bg-surface text-muted grid size-36 place-items-center rounded-full sm:size-44">
            <UserRound aria-hidden size={64} />
          </span>
        )}
        <div className="flex min-w-0 flex-col items-center gap-2 sm:items-start">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t('artist.kind')}
          </span>
          <h1 className="text-center text-3xl font-black tracking-tight sm:text-left sm:text-5xl">
            {own.data?.title ?? name}
          </h1>
          <p className="text-muted text-sm">
            {t('artist.summary', {
              popular: popular.length,
              library: overview.data?.tracks.length ?? 0,
              saved: saved.length,
            })}
          </p>
        </div>
      </header>
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          disabled={popular.length === 0 && (overview.data?.tracks.length ?? 0) === 0}
          onClick={() => play(false)}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold disabled:opacity-50"
        >
          <Play aria-hidden size={18} fill="currentColor" />
          {t('library.playAll')}
        </button>
        <button
          type="button"
          disabled={popular.length === 0 && (overview.data?.tracks.length ?? 0) === 0}
          onClick={() => play(true)}
          className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50"
        >
          <Shuffle aria-hidden size={18} />
          {t('library.shuffle')}
        </button>
        {/* Following pins the artist to quick access (M46.42), in place of a pin button. */}
        {name && <FollowButton name={name} imageUrl={picture} />}
      </div>
      {slow && (
        <LoaderCircle aria-label={t('artist.loading')} className="text-muted animate-spin" />
      )}
      {overview.error && <SourceIssue>{overview.error.message}</SourceIssue>}
      {popular.length > 0 && (
        <Section title={t('artist.popular')}>
          <ol className="flex flex-col">
            {popular.slice(0, more ? popular.length : POPULAR_SHOWN).map((track, index) => (
              <li key={`${track.title}-${index}`} className="flex items-center gap-3">
                <span className="text-muted w-5 shrink-0 text-right text-sm tabular-nums">
                  {index + 1}
                </span>
                <div className="min-w-0 flex-1">
                  <OnlineTrackRow
                    track={track}
                    onPlay={(tapped) => void playResults(popular, tapped)}
                  />
                </div>
              </li>
            ))}
          </ol>
          {popular.length > POPULAR_SHOWN && (
            <button
              type="button"
              onClick={() => setMore(!more)}
              className="text-muted hover:text-foreground self-start text-sm font-semibold"
            >
              {t(more ? 'artist.showLess' : 'artist.showMore')}
            </button>
          )}
        </Section>
      )}
      {overview.isSuccess && popular.length === 0 && (
        <p className="text-muted text-sm">{t('artist.noPopular')}</p>
      )}
      {name && <Discography name={name} title={t('artist.discography')} />}
      {name && <ArtistEvents name={name} />}
      {info.data && <AboutSection info={info.data} />}
      {(overview.data?.albums.length ?? 0) > 0 && (
        <Section title={t('artist.albums')}>
          <AlbumGrid albums={overview.data?.albums ?? []} />
        </Section>
      )}
      {(overview.data?.tracks.length ?? 0) > 0 && (
        <Section title={t('artist.inLibrary')}>
          <ul className="flex flex-col">
            {overview.data?.tracks.map((track) => (
              <li key={track.id}>
                <TrackRow
                  track={track}
                  onPlay={(tapped) =>
                    void playFromLibrary({
                      selection: { kind: 'artist', artist: name },
                      startTrackId: tapped.id,
                      shuffle: false,
                    })
                  }
                />
              </li>
            ))}
          </ul>
        </Section>
      )}
      {saved.length > 0 && (
        <Section title={t('artist.saved')}>
          <ul className="flex flex-col">
            {saved.map((track) => (
              <li key={track.trackKey}>
                <SavedTrackRow
                  track={track}
                  onPlay={() =>
                    void playCollection({
                      collection: { kind: 'favorites' },
                      startIndex: 0,
                      shuffle: false,
                      order: [
                        track.trackKey,
                        ...saved.filter((other) => other !== track).map((other) => other.trackKey),
                      ],
                    })
                  }
                />
              </li>
            ))}
          </ul>
        </Section>
      )}
    </section>
  );
};

/** What open databases say about the artist (M46.14): their story, where from, who's in it. */
const AboutSection: FC<{ info: ArtistInfo }> = ({ info }) => {
  const { t } = useTranslation();
  const facts = [info.kind, artistFacts(info)].filter(Boolean).join(' · ');
  const url = info.url;
  return (
    <Section title={t('artist.about')}>
      <div data-testid="artist-about" className="bg-surface flex flex-col gap-2 rounded-2xl p-4">
        {facts && <p className="text-muted text-sm">{facts}</p>}
        {info.bio && <p className="leading-relaxed whitespace-pre-line">{info.bio}</p>}
        {info.members.length > 0 && (
          <p className="text-muted text-sm">
            {t('artist.members', { names: info.members.join(', ') })}
          </p>
        )}
        <div className="text-muted flex items-center justify-between gap-3 text-xs">
          <span>{t('artist.from', { source: info.providerName })}</span>
          {url && (
            <button
              type="button"
              onClick={() => void commands.musicOpenInfoPage(url)}
              className="hover:text-foreground flex items-center gap-1 font-semibold"
            >
              <ExternalLink aria-hidden size={12} />
              {t('artist.readMore')}
            </button>
          )}
        </div>
      </div>
    </Section>
  );
};
