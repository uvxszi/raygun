import { commands } from '@raygun/bindings';
import type { LyricLine, QueueItem, SpokenItem } from '@raygun/bindings';
import { Link, useNavigate } from '@tanstack/react-router';
import { ExternalLink, Maximize2, UserRound } from 'lucide-react';
import { memo, useId, useMemo, useRef, useState } from 'react';
import type { FC, Ref } from 'react';
import { useTranslation } from 'react-i18next';

import { creditedArtists } from '../music/artists';
import { Sheet } from '../shell/Sheet';
import { CaptionsView } from './CaptionsView';
import { trackQueryFor, useLyricsFor } from './lyricsQuery';
import { artistFacts, shownCredits, useArtistInfo, useSongCredits } from './musicInfo';
import { useLyricsLook } from './playerView';
import { useCurrentLine, useFollowLine } from './useFollowLine';

/** Lines of plain lyrics the card shows. */
const PLAIN_LINES = 5;

const NO_LINES: LyricLine[] = [];

/** Credit roles the card shows before "Show all". */
const CREDITS_SHOWN = 3;

// Cards keep their height in a scrolling column (M46.24: shrinking cut their last lines).
const CARD = 'flex w-full shrink-0 flex-col gap-3 overflow-hidden rounded-2xl text-left';

type PlayerCardsProps = {
  item: QueueItem;
  /** A podcast episode or a book's chapter: captions instead of lyrics, artist and credits. */
  spoken?: SpokenItem | null;
  /** Shows the lyrics full screen. */
  onLyrics: () => void;
};

/**
 * What sits under the phone's player (M46.14), as in Spotify: the lyrics, the artist and who made
 * the song. Each card opens more: the lyrics full screen, the artist's page, all the credits.
 */
export const PlayerCards: FC<PlayerCardsProps> = ({ item, spoken, onLyrics }) => (
  <div data-testid="player-cards" className="flex w-full flex-col gap-4 pt-8">
    {spoken ? (
      <CaptionsCard spoken={spoken} onOpen={onLyrics} />
    ) : (
      <>
        <LyricsCard item={item} onOpen={onLyrics} />
        <ArtistCard credit={item.artist} />
        <CreditsCard item={item} />
      </>
    )}
  </div>
);

/** Captions for spoken items (M46.32), slim until there are lines. */
const CaptionsCard: FC<{
  spoken: SpokenItem;
  onOpen: () => void;
}> = ({ spoken, onOpen }) => {
  const { t } = useTranslation();
  const heading = useId();
  return (
    <section
      aria-labelledby={heading}
      data-testid="captions-card"
      className={`${CARD} bg-surface gap-0 pt-3`}
    >
      <header className="flex items-center justify-between px-4">
        <h3 id={heading} className="font-bold">
          {t('captions.title')}
        </h3>
        <button
          type="button"
          aria-label={t('captions.full')}
          onClick={onOpen}
          className="text-muted hover:text-foreground grid size-8 place-items-center rounded-full"
        >
          <Maximize2 aria-hidden size={16} />
        </button>
      </header>
      <CaptionsView spoken={spoken} variant="panel" />
    </section>
  );
};

const LyricsCard: FC<{ item: QueueItem; onOpen: () => void }> = ({ item, onOpen }) => {
  const { t } = useTranslation();
  const heading = useId();
  const lyrics = useLyricsFor(trackQueryFor(item));
  const { style } = useLyricsLook();
  const synced = lyrics.data?.synced ?? NO_LINES;
  const plain = synced.length === 0 ? (lyrics.data?.plain ?? '') : '';
  const plainLines = useMemo(
    () =>
      plain
        .split('\n')
        .filter((line) => line.trim() !== '')
        .slice(0, PLAIN_LINES),
    [plain],
  );

  const note = lyrics.isPending
    ? t('player.lyricsLoading')
    : lyrics.error
      ? t('player.cards.lyricsFailed')
      : synced.length === 0 && plainLines.length === 0
        ? t('player.cards.lyricsNone')
        : null;

  // Nothing to show: one slim line instead of a tall card (M46.24).
  if (note) {
    return (
      <button
        type="button"
        aria-labelledby={heading}
        data-testid="lyrics-card"
        data-empty="true"
        onClick={onOpen}
        className={`${CARD} bg-surface flex-row items-center gap-3 px-4 py-3`}
      >
        <span id={heading} className="shrink-0 font-bold">
          {t('player.lyrics')}
        </span>
        <span className="text-muted min-w-0 flex-1 truncate text-sm">{note}</span>
        <Maximize2 aria-hidden size={16} className="text-muted shrink-0" />
      </button>
    );
  }

  return (
    <button
      type="button"
      aria-labelledby={heading}
      data-testid="lyrics-card"
      onClick={onOpen}
      style={style}
      className={`${CARD} from-primary/40 via-primary/20 to-surface @container bg-gradient-to-br p-4`}
    >
      <span className="flex w-full items-center justify-between">
        <span id={heading} className="font-bold">
          {t('player.lyrics')}
        </span>
        <Maximize2 aria-hidden size={18} className="text-muted" />
      </span>
      {synced.length > 0 ? (
        <CardLines lines={synced} offsetMs={lyrics.data?.offsetMs ?? 0} />
      ) : (
        // Plain lyrics don't move: their first lines.
        <span className="flex flex-col gap-[0.4em] leading-snug font-bold">
          {plainLines.map((text, index) => (
            <span key={index} data-variant="panel" className="lyric-text line-clamp-2">
              {text}
            </span>
          ))}
        </span>
      )}
    </button>
  );
};

/**
 * The card's synced lines in a window a few lines tall (styles.css `.lyrics-card-window`), led by
 * the line sung. As the next one starts the window glides to it as the full lyrics do
 * (useFollowLine), the line sung easing in (`.lyric-line`) while the one before leaves at the top
 * and the next ones fade in at the bottom.
 */
const CardLines: FC<{ lines: LyricLine[]; offsetMs: number }> = ({ lines, offsetMs }) => {
  const box = useRef<HTMLSpanElement>(null);
  // Follows the playhead, re-rendering only as a line starts.
  const starts = useMemo(() => lines.map((line) => line.timeMs + offsetMs), [lines, offsetMs]);
  const current = useCurrentLine(starts);
  const { lineRef, listRef } = useFollowLine<HTMLSpanElement, HTMLSpanElement>(
    box,
    current,
    lines,
    { place: 'top', scrollable: false },
  );
  return (
    <span
      ref={box}
      data-testid="lyrics-card-lines"
      data-variant="panel"
      className="lyric-text lyrics-card-window leading-snug font-bold"
    >
      <span ref={listRef} className="flex flex-col gap-[0.4em]">
        {lines.map((line, index) => (
          <CardLine
            key={`${line.timeMs}-${index}`}
            text={line.text || '♪'}
            playing={index === current}
            lineRef={index === current ? lineRef : undefined}
          />
        ))}
      </span>
    </span>
  );
};

/** One of the card's lines; it re-renders only when it starts or stops being the one sung. */
const CardLine: FC<{ text: string; playing: boolean; lineRef?: Ref<HTMLSpanElement> }> = memo(
  function CardLine({ text, playing, lineRef }) {
    return (
      <span
        ref={lineRef}
        data-variant="panel"
        data-current={playing ? '' : undefined}
        className="lyric-text lyric-line text-foreground line-clamp-2"
      >
        {text}
      </span>
    );
  },
);

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
        {/* Two lines at most, a step smaller in a narrow column (the side panel), never cut. */}
        <span className="card-title flex items-center gap-2 font-bold">
          {!image && <UserRound aria-hidden size={18} className="text-muted shrink-0" />}
          <span className="line-clamp-2 min-w-0 leading-tight wrap-anywhere">
            {info.data?.name ?? name}
          </span>
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
              {/* A narrow side panel shows two lines of names rather than one cut short. */}
              <span className="truncate font-medium @max-xs/panel:line-clamp-2 @max-xs/panel:whitespace-normal">
                {group.names.join(', ')}
              </span>
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
