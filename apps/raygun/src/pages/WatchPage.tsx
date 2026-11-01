import { Link } from '@tanstack/react-router';
import { Clapperboard, LoaderCircle, Play, Puzzle, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useContinueWatching,
  usePlayLink,
  useRemoveContinue,
  useResumeVideo,
} from '../watch/useWatch';
import { StremioRow } from '../watch/StremioRow';
import { useStremioHome } from '../watch/useStremio';
import { PageHeader } from './PageHeader';

const Poster: FC<{ url: string | null }> = ({ url }) =>
  url ? (
    <img src={url} alt="" loading="lazy" className="bg-surface aspect-video w-full object-cover" />
  ) : (
    <span aria-hidden className="bg-surface text-muted grid aspect-video w-full place-items-center">
      <Clapperboard size={28} />
    </span>
  );

/** The Watch tab: continue watching, catalogs from Stremio addons, and playing links. */
export const WatchPage: FC = () => {
  const { t } = useTranslation();
  const continuing = useContinueWatching();
  const resume = useResumeVideo();
  const remove = useRemoveContinue();
  const playLink = usePlayLink();
  const home = useStremioHome();
  const [url, setUrl] = useState('');
  const [subtitleUrl, setSubtitleUrl] = useState('');

  const onPlayLink = (event: FormEvent) => {
    event.preventDefault();
    if (url.trim()) {
      playLink.mutate({ url: url.trim(), subtitleUrl: subtitleUrl.trim() || null });
    }
  };
  const error = resume.error ?? playLink.error;

  return (
    <section data-testid="watch-page" className="flex flex-col gap-5">
      <div className="flex items-center justify-between gap-3">
        <PageHeader title={t('watch.title')} />
        <Link
          to="/watch/addons"
          className="bg-surface flex items-center gap-1.5 rounded-full px-4 py-1.5 text-sm"
        >
          <Puzzle aria-hidden size={16} />
          {t('watch.addons')}
        </Link>
      </div>
      {(continuing.data?.length ?? 0) > 0 && (
        <section aria-labelledby="watch-continue" className="flex flex-col gap-2">
          <h2 id="watch-continue" className="font-semibold">
            {t('watch.continue')}
          </h2>
          <ul className="-mx-4 flex gap-3 overflow-x-auto px-4 lg:mx-0 lg:px-0">
            {continuing.data?.map((item) => {
              const duration = item.durationMs ?? 0;
              const position = item.positionMs ?? 0;
              return (
                <li key={item.key} aria-label={item.title} className="relative w-48 shrink-0">
                  <button
                    type="button"
                    onClick={() => resume.mutate(item.key)}
                    className="flex w-full flex-col gap-1 overflow-hidden text-left"
                  >
                    <span className="relative overflow-hidden rounded-lg">
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
                        <span className="grid size-10 place-items-center rounded-full bg-black/50 text-white">
                          <Play aria-hidden size={18} />
                        </span>
                      </span>
                    </span>
                    <span className="line-clamp-1 text-sm font-medium">{item.title}</span>
                    {item.subtitle && (
                      <span className="text-muted line-clamp-1 text-xs">{item.subtitle}</span>
                    )}
                  </button>
                  <button
                    type="button"
                    aria-label={t('watch.removeContinue', { title: item.title })}
                    onClick={() => remove.mutate(item.key)}
                    className="absolute top-1 right-1 grid size-7 place-items-center rounded-full bg-black/50 text-white"
                  >
                    <X aria-hidden size={14} />
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      )}
      {home.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {home.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {home.error.message}
        </p>
      )}
      {home.data?.length === 0 && (
        <p className="text-muted text-sm">
          {t('watch.noCatalogs')}{' '}
          <Link to="/watch/addons" className="text-primary">
            {t('watch.addSomeAddons')}
          </Link>
        </p>
      )}
      {home.data?.map((row) => (
        <StremioRow key={`${row.addonId}-${row.kind}-${row.catalogId}`} row={row} />
      ))}
      <section
        aria-labelledby="watch-link"
        className="bg-surface flex flex-col gap-2 rounded-xl px-5 py-4 text-sm"
      >
        <h2 id="watch-link" className="font-semibold">
          {t('watch.playLink')}
        </h2>
        <p className="text-muted text-xs">{t('watch.playLinkHint')}</p>
        <form onSubmit={onPlayLink} className="flex flex-col gap-2">
          <input
            type="url"
            inputMode="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            placeholder="https://…/video.m3u8"
            aria-label={t('watch.videoUrl')}
            className="bg-background min-w-0 rounded-lg px-3 py-2 outline-none"
          />
          <input
            type="url"
            inputMode="url"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            value={subtitleUrl}
            onChange={(event) => setSubtitleUrl(event.target.value)}
            placeholder={t('watch.subtitlePlaceholder')}
            aria-label={t('watch.subtitleUrl')}
            className="bg-background min-w-0 rounded-lg px-3 py-2 outline-none"
          />
          <button
            type="submit"
            disabled={!url.trim() || playLink.isPending}
            className="bg-primary text-background flex w-fit items-center gap-2 rounded-full px-4 py-2 font-semibold disabled:opacity-60"
          >
            <Play aria-hidden size={16} />
            {t('watch.play')}
          </button>
        </form>
      </section>
      {error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {error.message}
        </p>
      )}
    </section>
  );
};
