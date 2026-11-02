import { commands } from '@raygun/bindings';
import type { PlaybackState, QueueItem } from '@raygun/bindings';
import { Link, useNavigate } from '@tanstack/react-router';
import { ExternalLink, Maximize2, UserRound } from 'lucide-react';
import { useId, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { creditedArtists } from '../music/artists';
import { Sheet } from '../shell/Sheet';
import { currentLine, trackQueryFor, useLyricsFor } from './lyricsQuery';
import { artistFacts, shownCredits, useArtistInfo, useSongCredits } from './musicInfo';
import { useLivePosition } from './useLivePosition';

/** Lines of lyrics the card shows. */
const PREVIEW_LINES = 5;

/** Credit roles the card shows before "Show all". */
const CREDITS_SHOWN = 3;

const CARD = 'flex w-full flex-col gap-3 overflow-hidden rounded-2xl text-left';

type PlayerCardsProps = {
  item: QueueItem;
  state: PlaybackState | undefined;
  /** Shows the lyrics full screen. */
  onLyrics: () => void;
};

/**
 * What sits under the phone's player (M46.14), as in Spotify: the lyrics, the artist and who made
 * the song. Each card opens more: the lyrics full screen, the artist's page, all the credits.
 */
export const PlayerCards: FC<PlayerCardsProps> = ({ item, state, onLyrics }) => (
  <div data-testid="player-cards" className="mx-auto flex w-full max-w-xl flex-col gap-4 pt-8">
    <LyricsCard item={item} state={state} onOpen={onLyrics} />
    <ArtistCard credit={item.artist} />
    <CreditsCard item={item} />
  </div>
);

const LyricsCard: FC<{ item: QueueItem; state: PlaybackState | undefined; onOpen: () => void }> = ({
  item,
  state,
  onOpen,
}) => {
  const { t } = useTranslation();
  const heading = useId();
  const lyrics = useLyricsFor(trackQueryFor(item));
  const positionMs = useLivePosition(state);
  const synced = lyrics.data?.synced ?? [];
  const current = currentLine(synced, positionMs, lyrics.data?.offsetMs ?? 0);
  // Synced lyrics show from the line playing; plain ones from the top.
  const start = Math.max(0, current);
  const lines =
    synced.length > 0
      ? synced.slice(start, start + PREVIEW_LINES).map((line, index) => ({
          key: `${line.timeMs}-${start + index}`,
          text: line.text || '♪',
          playing: start + index === current,
        }))
      : (lyrics.data?.plain ?? '')
          .split('\n')
          .filter((line) => line.trim() !== '')
          .slice(0, PREVIEW_LINES)
          .map((text, index) => ({ key: `${index}`, text, playing: false }));

  const note = lyrics.isPending
    ? t('player.lyricsLoading')
    : lyrics.error
      ? t('player.cards.lyricsFailed')
      : lines.length === 0
        ? t('player.cards.lyricsNone')
        : null;

  return (
    <button
      type="button"
      aria-labelledby={heading}
      data-testid="lyrics-card"
      onClick={onOpen}
      className={`${CARD} from-primary/40 via-primary/20 to-surface bg-gradient-to-br p-4`}
    >
      <span className="flex w-full items-center justify-between">
        <span id={heading} className="font-bold">
          {t('player.lyrics')}
        </span>
        <Maximize2 aria-hidden size={18} className="text-muted" />
      </span>
      {note ? (
        <span className="text-muted text-sm">{note}</span>
      ) : (
        <span className="flex flex-col gap-1.5 text-xl leading-snug font-bold">
          {lines.map((line) => (
            <span
              key={line.key}
              className={`line-clamp-2 transition-colors ${
                line.playing || synced.length === 0 ? 'text-foreground' : 'text-foreground/45'
              }`}
            >
              {line.text}
            </span>
          ))}
        </span>
      )}
    </button>
  );
};

/** About the artist (M46.14): also below the desktop side panel's Now playing. */
export const ArtistCard: FC<{ credit: string | null }> = ({ credit }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const name = creditedArtists(credit)[0];
  const info = useArtistInfo(name);
  if (!name) return null;
  const facts = artistFacts(info.data);
  const image = info.data?.imageUrl;

  return (
    <button
      type="button"
      aria-label={t('player.cards.aboutArtistNamed', { name })}
      data-testid="artist-card"
      // The player closes once the page changes (closing first would undo the navigation).
      onClick={() => void navigate({ to: '/artist', search: { name } })}
      className={`${CARD} bg-surface`}
    >
      <span className="relative block w-full">
        {image ? (
          <img src={image} alt="" className="aspect-[16/10] w-full object-cover object-top" />
        ) : (
          <span className="block h-12" />
        )}
        {image && (
          <span className="absolute inset-0 bg-gradient-to-b from-black/50 via-transparent to-transparent" />
        )}
        <span
          className={`absolute top-3 left-4 font-bold ${image ? 'text-white drop-shadow' : ''}`}
        >
          {t('player.cards.aboutArtist')}
        </span>
      </span>
      <span className="flex flex-col gap-1 px-4 pb-4">
        <span className="flex items-center gap-2 text-lg font-bold">
          {!image && <UserRound aria-hidden size={18} className="text-muted shrink-0" />}
          <span className="truncate">{info.data?.name ?? name}</span>
        </span>
        {facts && <span className="text-muted text-sm">{facts}</span>}
        {info.isPending ? (
          <span className="text-muted text-sm">{t('player.cards.artistLoading')}</span>
        ) : info.data?.bio ? (
          <span className="text-muted line-clamp-3 text-sm">{info.data.bio}</span>
        ) : (
          <span className="text-muted text-sm">{t('player.cards.artistOpen')}</span>
        )}
      </span>
    </button>
  );
};

/** Who made the song (M46.14): also below the desktop side panel's Now playing. */
export const CreditsCard: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const heading = useId();
  const [open, setOpen] = useState(false);
  const credits = useSongCredits(trackQueryFor(item));
  const groups = shownCredits(credits.data, item, t('player.cards.performedBy'));
  if (groups.length === 0 && !credits.isPending) return null;

  return (
    <>
      <button
        type="button"
        aria-labelledby={heading}
        aria-haspopup="dialog"
        data-testid="credits-card"
        onClick={() => setOpen(true)}
        className={`${CARD} bg-surface p-4`}
      >
        <span className="flex w-full items-center justify-between">
          <span id={heading} className="font-bold">
            {t('player.cards.credits')}
          </span>
          <span className="text-muted text-sm font-semibold">{t('player.cards.showAll')}</span>
        </span>
        <span className="flex flex-col gap-3">
          {groups.slice(0, CREDITS_SHOWN).map((group) => (
            <span key={group.role} className="flex flex-col">
              <span className="truncate font-medium">{group.names.join(', ')}</span>
              <span className="text-muted text-sm">{group.role}</span>
            </span>
          ))}
          {credits.isPending && (
            <span className="text-muted text-sm">{t('player.cards.creditsLoading')}</span>
          )}
        </span>
      </button>
      {open && (
        <Sheet title={t('player.cards.credits')} onClose={() => setOpen(false)}>
          <CreditsList item={item} />
        </Sheet>
      )}
    </>
  );
};

/** Every credit, each name leading to that artist's page, and where they came from. */
const CreditsList: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const credits = useSongCredits(trackQueryFor(item));
  const groups = shownCredits(credits.data, item, t('player.cards.performedBy'));
  const url = credits.data?.url;

  return (
    <div data-testid="credits-list" className="flex flex-col gap-4 pb-2">
      <p className="truncate font-semibold">{item.title}</p>
      {groups.map((group) => (
        <section key={group.role} aria-label={group.role} className="flex flex-col gap-1">
          <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">{group.role}</h3>
          <ul className="flex flex-col">
            {group.names.map((name) => (
              <li key={name}>
                <Link
                  to="/artist"
                  search={{ name }}
                  className="hover:bg-surface/60 block rounded-lg py-1.5 font-medium hover:underline"
                >
                  {name}
                </Link>
              </li>
            ))}
          </ul>
        </section>
      ))}
      {credits.data && (
        <div className="text-muted flex items-center justify-between gap-3 text-xs">
          <span>{t('player.cards.from', { source: credits.data.providerName })}</span>
          {url && (
            <button
              type="button"
              onClick={() => void commands.musicOpenInfoPage(url)}
              className="hover:text-foreground flex items-center gap-1 font-semibold"
            >
              <ExternalLink aria-hidden size={12} />
              {t('player.cards.openSource', { source: credits.data.providerName })}
            </button>
          )}
        </div>
      )}
    </div>
  );
};
