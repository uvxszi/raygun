import type { ContinueWatching } from '@raygun/bindings';
import { Play, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useContinueWatching, useRemoveContinue, useResumeVideo } from './useWatch';
import { Carousel } from '../shell/Carousel';

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

const Card: FC<{ item: ContinueWatching }> = ({ item }) => {
  const { t } = useTranslation();
  const resume = useResumeVideo();
  const remove = useRemoveContinue();
  const duration = item.durationMs ?? 0;
  const position = item.positionMs ?? 0;
  return (
    <li aria-label={item.title} className="relative w-56 shrink-0">
      <button
        type="button"
        onClick={() => resume.mutate(item.key)}
        className="flex w-full flex-col gap-1.5 overflow-hidden text-left"
      >
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
          <span className="absolute inset-0 grid place-items-center">
            <span className="grid size-11 place-items-center rounded-full bg-black/55 text-white">
              <Play aria-hidden size={20} fill="currentColor" />
            </span>
          </span>
        </span>
        <span className="line-clamp-1 text-sm font-medium">{item.title}</span>
        {item.subtitle && <span className="text-muted line-clamp-1 text-xs">{item.subtitle}</span>}
      </button>
      <button
        type="button"
        aria-label={t('watch.removeContinue', { title: item.title })}
        onClick={() => remove.mutate(item.key)}
        className="absolute top-1.5 right-1.5 grid size-8 place-items-center rounded-full bg-black/55 text-white"
      >
        <X aria-hidden size={15} />
      </button>
      {resume.error && (
        <p role="alert" className="text-primary mt-1 text-xs wrap-anywhere">
          {resume.error.message}
        </p>
      )}
    </li>
  );
};

/** Videos started and not finished, as a sideways row; nothing when there are none. */
export const ContinueWatchingRow: FC = () => {
  const { t } = useTranslation();
  const continuing = useContinueWatching();
  if (!continuing.data?.length) return null;
  return (
    <section aria-labelledby="continue-watching" className="flex flex-col gap-2">
      <h2 id="continue-watching" className="text-lg font-semibold">
        {t('watch.continue')}
      </h2>
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
        {continuing.data.map((item) => (
          <Card key={item.key} item={item} />
        ))}
      </Carousel>
    </section>
  );
};
