import { Play } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useCoverHold } from '../music/useCoverHold';
import { AdultMark } from './AdultMark';
import { Picture } from './Picture';
import { SourceTag } from './SourceTag';

/**
 * A title screen's header (Mihon's and CloudStream's): a blurred copy of the art behind the
 * cover, title, facts and genres. Its pictures fade in, and a better one (the details' cover
 * after the list's) fades in over the one shown (shell/Picture).
 */
export const TitleHero: FC<{
  art: string | null;
  /** A wide backdrop shown sharp above the header (movies and series). */
  backdrop?: string | null;
  /**
   * A backdrop may still come (the details are loading): its room is kept, the art blurred in
   * it, so the header doesn't jump down when it comes, and it fades in there.
   */
  backdropPending?: boolean;
  cover: ReactNode;
  title: string;
  facts?: (string | number | null | undefined | false)[];
  /** Shown after the facts, e.g. ratings that open their sources (M46.45). */
  factsExtra?: ReactNode;
  /** A wide 16:9 cover (M46.45: video sites' thumbnails) gets a wider column. */
  wideCover?: boolean;
  genres?: string[];
  genresLabel?: string;
  children?: ReactNode;
  /** Shown over the backdrop's right side on wide windows, under the title on phones. */
  aside?: ReactNode;
  /** Adult genres or a source rated for adults: an 18+ tag after the title. */
  adult?: boolean;
  /** Where it comes from (a source, an addon): a tag of its own, never in the facts' line. */
  source?: string | null;
}> = ({
  art,
  backdrop,
  backdropPending = false,
  cover,
  title,
  facts = [],
  factsExtra,
  wideCover = false,
  genres = [],
  genresLabel,
  children,
  aside,
  adult = false,
  source = null,
}) => {
  const { t } = useTranslation();
  const shown = facts.filter(Boolean);
  // The cover and the backdrop open close up, to zoom and save, when held or right-clicked.
  const coverHold = useCoverHold({ cover: art ? { url: art, title } : null });
  const backdropHold = useCoverHold({
    cover: backdrop ? { url: backdrop, title: t('cover.backdropName', { title }) } : null,
  });
  const wide = Boolean(backdrop) || backdropPending;
  const blurredArt = art && (
    <Picture src={art} eager className="size-full scale-110 opacity-35 blur-2xl" />
  );
  return (
    <div className="relative isolate -mx-4 -mt-2 overflow-hidden px-4 pt-2 pb-4 lg:-mx-10 lg:px-10">
      {wide && (
        // At most about half the window (M46.34), so the title, facts and Play show at once.
        <div
          aria-hidden
          data-testid={backdrop ? 'title-backdrop' : 'title-backdrop-pending'}
          {...backdropHold.props}
          className="cover-hold relative -mx-4 -mt-2 h-[min(56.25vw,52vh)] lg:-mx-10"
        >
          {/* The art, blurred, under the backdrop until it has faded in. */}
          <Picture
            src={backdrop}
            eager
            fit="object-cover object-[50%_25%]"
            placeholder={blurredArt}
            keepPlaceholder
            className="size-full"
          />
          <div className="to-background absolute inset-0 bg-gradient-to-b from-transparent from-30%" />
        </div>
      )}
      {art && !wide && (
        <div aria-hidden className="absolute inset-0 -z-10">
          {blurredArt}
          <div className="from-background/40 to-background absolute inset-0 bg-gradient-to-b" />
        </div>
      )}
      <header className={`relative flex gap-4 ${wide ? '-mt-20' : ''}`}>
        <div className={`shrink-0 ${wideCover ? 'w-40 sm:w-64' : 'w-28 sm:w-36'}`}>
          {art ? (
            <button
              type="button"
              aria-label={t('cover.open')}
              title={t('cover.holdHint')}
              {...coverHold.props}
              className="cover-hold block w-full"
            >
              {cover}
            </button>
          ) : (
            cover
          )}
        </div>
        <div className="flex min-w-0 flex-col justify-end gap-2">
          <h1 className="text-xl leading-tight font-bold tracking-tight wrap-anywhere sm:text-2xl">
            {title}
            {adult && <AdultMark className="ml-2" />}
          </h1>
          {(shown.length > 0 || factsExtra) && (
            <p className="text-muted flex flex-wrap items-center gap-x-1.5 text-sm">
              {shown.join(' · ')}
              {factsExtra && shown.length > 0 && <span aria-hidden>·</span>}
              {factsExtra}
            </p>
          )}
          {source && (
            <span className="flex min-w-0">
              <SourceTag name={source} className="bg-surface/60" />
            </span>
          )}
          {genres.length > 0 && (
            <ul aria-label={genresLabel} className="flex flex-wrap gap-1.5">
              {genres.slice(0, 6).map((genre) => (
                <li key={genre} className="bg-surface/80 rounded-full px-2.5 py-0.5 text-xs">
                  {genre}
                </li>
              ))}
            </ul>
          )}
        </div>
      </header>
      {children && <div className="mt-4">{children}</div>}
      {aside && (
        <div
          className={
            backdrop
              ? 'mt-4 md:absolute md:top-6 md:right-10 md:mt-0 md:w-[min(26rem,36%)]'
              : 'mt-4'
          }
        >
          {aside}
        </div>
      )}
    </div>
  );
};

/**
 * The floating Play / Resume button (Mihon's "Start" and "Resume"), kept at the bottom right of
 * the screen while the page scrolls. Render it last in the page.
 */
export const ResumeFab: FC<{ label: string; onClick: () => void; icon?: LucideIcon }> = ({
  label,
  onClick,
  icon: Icon = Play,
}) => (
  <div className="pointer-events-none sticky bottom-4 z-20 mt-auto flex justify-end pt-4">
    <button
      type="button"
      onClick={onClick}
      className="bg-primary text-background pointer-events-auto flex items-center gap-2 rounded-2xl px-5 py-4 font-semibold shadow-lg shadow-black/30"
    >
      <Icon aria-hidden size={20} fill="currentColor" />
      {label}
    </button>
  </div>
);
