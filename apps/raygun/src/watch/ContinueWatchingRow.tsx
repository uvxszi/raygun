import type { ContinueWatching } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Play, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useContinueWatching, useRemoveContinue, useResumeVideo } from './useWatch';
import { videoTitleLink } from './useVideoLibrary';
import { CardPlayButton } from '../manga/CardPlayButton';
import { useTitleCardMenu } from '../manga/titleMenu';
import { buildMenu } from '../music/mediaMenu';
import { Carousel } from '../shell/Carousel';
import type { MenuItem } from '../shell/contextMenuStore';
import { MenuButton } from '../shell/MenuButton';
import { SourceIssue } from '../shell/SourceIssue';

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
 * title's page (a link played on its own, with no page, resumes instead). ⋮, a right click or a
 * long press opens the title's menu with Resume and Remove from Continue watching.
 */
const Card: FC<{ item: ContinueWatching }> = ({ item }) => {
  const { t } = useTranslation();
  const resume = useResumeVideo();
  const remove = useRemoveContinue();
  const titleMenu = useTitleCardMenu();
  const duration = item.durationMs ?? 0;
  const position = item.positionMs ?? 0;
  const page = item.sourceId && item.remoteId ? videoTitleLink(item.sourceId, item.remoteId) : null;
  const resumeItem: MenuItem = {
    label: t('watch.resume'),
    icon: Play,
    onSelect: () => resume.mutate(item.key),
  };
  const removeItem: MenuItem = {
    label: t('watch.removeContinue', { title: item.title }),
    icon: X,
    onSelect: () => remove.mutate(item.key),
  };
  const menu = (): MenuItem[] => {
    if (!item.sourceId || !item.remoteId) {
      return buildMenu({
        heading: { label: item.title, subtitle: item.subtitle, artworkUrl: item.poster },
        play: [resumeItem],
        remove: [removeItem],
      });
    }
    // The title's menu, with Resume (this episode where it stopped) in Play's place.
    const items = titleMenu({
      medium: 'video',
      sourceId: item.sourceId,
      id: item.remoteId,
      title: item.title,
      coverUrl: item.poster,
      subtitle: item.subtitle,
    });
    const playAt = items.findIndex((entry) => entry.kind !== 'heading');
    return [
      ...items.slice(0, playAt),
      resumeItem,
      ...items.slice(playAt + 1),
      { kind: 'separator' },
      removeItem,
    ];
  };
  const body: ReactNode = (
    <>
      <span className="relative overflow-hidden rounded-xl">
        <Poster url={item.poster} />
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
    <li aria-label={item.title} className="relative w-56 shrink-0">
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
      <span className="absolute top-1.5 right-1.5 flex gap-1">
        <MenuButton
          label={t('menu.options', { name: item.title })}
          items={menu}
          className="grid size-8 place-items-center rounded-full bg-black/55 text-white"
          iconSize={15}
        />
        <button
          type="button"
          aria-label={t('watch.removeContinue', { title: item.title })}
          onClick={() => remove.mutate(item.key)}
          className="grid size-8 place-items-center rounded-full bg-black/55 text-white"
        >
          <X aria-hidden size={15} />
        </button>
      </span>
      {resume.error && <SourceIssue compact>{resume.error.message}</SourceIssue>}
    </li>
  );
};

/**
 * Videos started and not finished, as a sideways row; nothing when there are none. `menu` puts
 * a ⋯ beside its heading (Home's Hide).
 */
export const ContinueWatchingRow: FC<{ menu?: () => MenuItem[] }> = ({ menu }) => {
  const { t } = useTranslation();
  const continuing = useContinueWatching();
  if (!continuing.data?.length) return null;
  return (
    <section aria-labelledby="continue-watching" className="flex flex-col gap-2">
      <div className="flex min-h-9 items-center gap-3">
        <h2 id="continue-watching" className="text-lg font-semibold">
          {t('watch.continue')}
        </h2>
        {menu && (
          <MenuButton
            horizontal
            label={t('menu.more', { name: t('watch.continue') })}
            items={menu}
            className="text-muted hover:text-foreground hover:bg-surface ml-auto grid size-8 place-items-center rounded-full"
          />
        )}
      </div>
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
        {continuing.data.map((item) => (
          <Card key={item.key} item={item} />
        ))}
      </Carousel>
    </section>
  );
};
