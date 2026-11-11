import type { ContinueWatching } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { CARD_MENU_CLASS, CardPlayButton } from '../manga/CardPlayButton';
import { Carousel } from '../shell/Carousel';
import type { MenuItem } from '../shell/contextMenuStore';
import { MenuButton } from '../shell/MenuButton';
import { SourceIssue } from '../shell/SourceIssue';
import { entryPage } from './continueEntry';
import { useContinueMenu } from './continueMenu';
import { ContinueSelectionBar, PickCircle } from './ContinueSelection';
import { ContinueSourcePicker } from './ContinueSourcePicker';
import { useContinueSelection, usePickGestures } from './useContinueSelection';
import type { ContinueSelection } from './useContinueSelection';
import { useContinueWatching, useResumeVideo } from './useWatch';

/** The poster, or a placeholder when there's none or it can't load (offline). */
const Poster: FC<{ url: string | null }> = ({ url }) => {
  const [failed, setFailed] = useState(false);
  return url && !failed ? (
    <img
      src={url}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
      className="bg-surface aspect-video w-full object-cover"
    />
  ) : (
    // The card's play button sits in the middle, so the placeholder stays plain.
    <span aria-hidden className="bg-surface block aspect-video w-full" />
  );
};

/**
 * A video started and not finished: the play button resumes it, the rest of the card opens its
 * title's page (a link played on its own opens its own), and ⋮ has the rest (removing it is
 * there, asked first). A long press, its circle or Ctrl/Shift-click picks it.
 */
const Card: FC<{
  item: ContinueWatching;
  selection: ContinueSelection;
  menu: (item: ContinueWatching) => MenuItem[];
}> = ({ item, selection, menu }) => {
  const { t } = useTranslation();
  const resume = useResumeVideo();
  const selected = selection.isSelected(item.key);
  const pick = (range: boolean) => selection.pick(item.key, range);
  const gestures = usePickGestures(selection.selecting, pick);
  const duration = item.durationMs ?? 0;
  const position = item.positionMs ?? 0;
  const page = entryPage(item);
  const body: ReactNode = (
    <>
      <span className="relative overflow-hidden rounded-xl">
        <Poster url={item.poster} />
        {selected && (
          <span
            aria-hidden
            className="bg-primary/20 ring-primary pointer-events-none absolute inset-0 rounded-xl ring-3 ring-inset"
          />
        )}
        {duration > 0 && (
          <span aria-hidden className="absolute inset-x-0 bottom-0 h-1 bg-black/40">
            <span
              className="bg-primary block h-full"
              style={{ width: `${Math.min(100, (position / duration) * 100)}%` }}
            />
          </span>
        )}
        <CardPlayButton
          label={t('watch.resumeTitle', { title: item.title })}
          onPlay={() => resume.mutate(item.key)}
          className="absolute top-1/2 left-1/2 size-11 -translate-x-1/2 -translate-y-1/2"
        />
      </span>
      <span className="line-clamp-1 text-sm font-medium">{item.title}</span>
      {item.subtitle && <span className="text-muted line-clamp-1 text-xs">{item.subtitle}</span>}
    </>
  );
  const cardClass = 'flex w-full flex-col gap-1.5 overflow-hidden text-left';
  return (
    <li
      aria-label={item.title}
      data-selected={selected || undefined}
      {...gestures}
      className="group/pick relative w-56 shrink-0 [-webkit-touch-callout:none]"
    >
      {page ? (
        <Link
          to={page.to}
          search={page.search as never}
          data-testid="continue-card"
          className={cardClass}
        >
          {body}
        </Link>
      ) : (
        <button
          type="button"
          data-testid="continue-card"
          onClick={() => resume.mutate(item.key)}
          className={cardClass}
        >
          {body}
        </button>
      )}
      <PickCircle checked={selected} shown={selection.selecting} name={item.title} onPick={pick} />
      {!selection.selecting && (
        <MenuButton
          label={t('menu.options', { name: item.title })}
          items={() => menu(item)}
          className={CARD_MENU_CLASS}
          iconSize={15}
        />
      )}
      {resume.error && <SourceIssue compact>{resume.error.message}</SourceIssue>}
    </li>
  );
};

/**
 * Videos started and not finished, as a sideways row; nothing when there are none. `menu` puts
 * a ⋯ beside its heading (Home's Hide). Picking cards turns the heading into the selection's bar.
 */
export const ContinueWatchingRow: FC<{ menu?: () => MenuItem[] }> = ({ menu }) => {
  const { t } = useTranslation();
  const continuing = useContinueWatching();
  const items = continuing.data ?? [];
  const selection = useContinueSelection(items.map((item) => item.key));
  const [choosing, setChoosing] = useState<ContinueWatching | null>(null);
  const cardMenu = useContinueMenu('continue', setChoosing);
  if (items.length === 0) return null;
  return (
    <section aria-label={t('watch.continue')} className="flex flex-col gap-2">
      {selection.selecting ? (
        <ContinueSelectionBar selection={selection} />
      ) : (
        <div className="flex min-h-9 items-center gap-3">
          <h2 className="text-lg font-semibold">{t('watch.continue')}</h2>
          {menu && (
            <MenuButton
              horizontal
              label={t('menu.more', { name: t('watch.continue') })}
              items={menu}
              className="text-muted hover:text-foreground hover:bg-surface ml-auto grid size-8 place-items-center rounded-full"
            />
          )}
        </div>
      )}
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
        {items.map((item) => (
          <Card key={item.key} item={item} selection={selection} menu={cardMenu} />
        ))}
      </Carousel>
      {choosing && <ContinueSourcePicker item={choosing} onClose={() => setChoosing(null)} />}
    </section>
  );
};
