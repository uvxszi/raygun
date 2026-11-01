import { Play } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC, ReactNode } from 'react';

/**
 * A title screen's header (Mihon's and CloudStream's): a blurred copy of the art behind the
 * cover, title, facts and genres.
 */
export const TitleHero: FC<{
  art: string | null;
  /** A wide backdrop shown sharp above the header (movies and series). */
  backdrop?: string | null;
  cover: ReactNode;
  title: string;
  facts?: (string | number | null | undefined | false)[];
  genres?: string[];
  genresLabel?: string;
  children?: ReactNode;
}> = ({ art, backdrop, cover, title, facts = [], genres = [], genresLabel, children }) => {
  const shown = facts.filter(Boolean);
  return (
    <div className="relative isolate -mx-4 -mt-2 overflow-hidden px-4 pt-2 pb-4 lg:-mx-10 lg:px-10">
      {backdrop && (
        <div aria-hidden className="relative -mx-4 -mt-2 aspect-video lg:-mx-10">
          <img src={backdrop} alt="" className="size-full object-cover" />
          <div className="to-background absolute inset-0 bg-gradient-to-b from-transparent from-40%" />
        </div>
      )}
      {art && !backdrop && (
        <div aria-hidden className="absolute inset-0 -z-10">
          <img src={art} alt="" className="size-full scale-110 object-cover opacity-35 blur-2xl" />
          <div className="from-background/40 to-background absolute inset-0 bg-gradient-to-b" />
        </div>
      )}
      <header className={`relative flex gap-4 ${backdrop ? '-mt-20' : ''}`}>
        <div className="w-28 shrink-0 sm:w-36">{cover}</div>
        <div className="flex min-w-0 flex-col justify-end gap-2">
          <h1 className="text-xl leading-tight font-bold tracking-tight wrap-anywhere sm:text-2xl">
            {title}
          </h1>
          {shown.length > 0 && <p className="text-muted text-sm">{shown.join(' · ')}</p>}
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
