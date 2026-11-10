import type { ExportConversion, ExportFormat, ExportRequest, ExportScope } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import type { TFunction } from 'i18next';
import { Check, CircleCheck, FolderOpen } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from '../manga/medium';
import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { withSave } from './saveDefaults';
import { SourceIssue } from '../shell/SourceIssue';
import {
  useExportFfmpeg,
  useExportPlan,
  useLastExportFolder,
  useLastExportFormat,
  usePickExportFolder,
  useStartExport,
} from './useExports';

/** Each medium's formats, the default first. Videos convert (MP4, MKV, the sound alone) or stay as downloaded (`video`). */
const FORMATS: Record<Medium, ExportFormat[]> = {
  manga: ['cbz', 'pdf', 'pdfPerChapter', 'images'],
  novel: ['epub', 'text'],
  video: ['mp4', 'mkv', 'video', 'm4a', 'mp3'],
};

/** `season` and `chosen` go to Rust as a list of chapters or episodes. */
type ScopeKind = 'all' | 'downloaded' | 'unread' | 'range' | 'season' | 'chosen';

/** Where the files go: the Settings folder, or the one last chosen here (`picked`). */
type Place = 'downloads' | 'settings' | 'picked' | 'share';

/** A chapter for the range pickers and its box, in reading order. */
export type ExportChapter = { id: string; name: string; downloaded?: boolean; read?: boolean };

/** An episode for the range pickers and its box, in watching order (ids: stored or the source's). */
export type ExportEpisode = {
  id: string;
  name: string;
  season: number | null;
  downloaded: boolean;
};

/** A chapter or an episode, as the scopes and boxes see them. */
type Unit = {
  id: string;
  name: string;
  season?: number | null;
  downloaded?: boolean;
  read?: boolean;
};

const Radio: FC<{
  name: string;
  checked: boolean;
  disabled?: boolean;
  onPick: () => void;
  children: ReactNode;
}> = ({ name, checked, disabled = false, onPick, children }) => (
  <button
    type="button"
    role="radio"
    aria-checked={checked}
    aria-label={name}
    disabled={disabled}
    onClick={onPick}
    className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-2.5 text-left disabled:opacity-50"
  >
    <span
      aria-hidden
      className={`grid size-5 shrink-0 place-items-center rounded-full border-2 ${checked ? 'border-primary' : 'border-muted'}`}
    >
      {checked && <span className="bg-primary size-2.5 rounded-full" />}
    </span>
    <span className="flex min-w-0 flex-col">{children}</span>
  </button>
);

const Group: FC<{ label: string; children: ReactNode }> = ({ label, children }) => (
  <div role="radiogroup" aria-label={label} className="-mx-2 flex flex-col">
    <h3 className="text-muted px-3 pt-1 text-xs font-semibold tracking-wide uppercase">{label}</h3>
    {children}
  </div>
);

const seasonName = (season: number | null, specials: string, named: (n: number) => string) =>
  season === null || season === 0 ? specials : named(season);

/** How long converting takes, in words ("about 4 minutes"). */
const timeText = (t: TFunction, seconds: number) => {
  if (seconds < 60) return t('export.convert.underMinute');
  if (seconds < 3600) return t('export.convert.minutes', { count: Math.ceil(seconds / 60) });
  return t('export.convert.hours', { count: Math.round(seconds / 3600) });
};

/**
 * What a video format does with the downloaded episodes: nothing re-encoded, or which streams are
 * re-encoded (from which codecs) and about how long it takes.
 */
const conversionText = (t: TFunction, conversion: ExportConversion) => {
  const what =
    conversion.video && conversion.audio
      ? 'both'
      : conversion.video
        ? 'picture'
        : conversion.audio
          ? 'sound'
          : null;
  if (!what) return t('export.convert.copies');
  const text =
    conversion.from.length > 0
      ? t(`export.convert.${what}From`, { from: conversion.from.join(', ') })
      : t(`export.convert.${what}`);
  return conversion.seconds === null
    ? text
    : t('export.convert.estimate', { what: text, time: timeText(t, conversion.seconds) });
};

/** The chapters or episodes a scope takes, as the sheet ticks them. */
const unitsIn = (
  units: Unit[],
  scope: ScopeKind,
  season: number | null,
  from: string,
  to: string,
  chosen: Set<string>,
): Set<string> => {
  switch (scope) {
    case 'downloaded':
      return new Set(units.filter((item) => item.downloaded).map((item) => item.id));
    case 'unread':
      return new Set(units.filter((item) => !item.read).map((item) => item.id));
    case 'season':
      return new Set(units.filter((item) => item.season === season).map((item) => item.id));
    case 'range': {
      const start = units.findIndex((item) => item.id === from);
      const end = units.findIndex((item) => item.id === to);
      if (start < 0 || end < 0) return new Set();
      return new Set(
        units.slice(Math.min(start, end), Math.max(start, end) + 1).map((item) => item.id),
      );
    }
    case 'chosen':
      return chosen;
    default:
      return new Set(units.map((item) => item.id));
  }
};

/**
 * Export a title (or a selection) as files to keep (M46.1): which chapters or episodes (all, a
 * range, or ticked ones), in what format, and where. Videos convert to MP4 or MKV, or keep only
 * their sound; every format takes every video, and each says whether it re-encodes the downloaded
 * episodes (and about how long that takes) or copies them. Re-encoding on a computer needs FFmpeg,
 * fetched here. The last format used is offered first. It says first how many must download,
 * which then go through the normal queue (episodes find their own links).
 */
export const ExportSheet: FC<{
  medium: Medium;
  titleIds: string[];
  /** One title's chapters in reading order: ranges and boxes per chapter. */
  chapters?: ExportChapter[];
  /** One title's episodes in watching order: seasons, ranges and boxes per episode. */
  episodes?: ExportEpisode[];
  /** Episodes ticked when the sheet opens (a selection on the title page). */
  initialEpisodes?: string[];
  /** Exactly `initialEpisodes`, with no choice of which (one saved video's menu). */
  fixed?: boolean;
  onClose: () => void;
}> = ({
  medium,
  titleIds,
  chapters = [],
  episodes = [],
  initialEpisodes,
  fixed = false,
  onClose,
}) => {
  const { t } = useTranslation();
  const save = withSave(useSettings().data?.save);
  const video = medium === 'video';
  const units: Unit[] = video ? episodes : chapters;
  const seasons = [...new Set(episodes.map((item) => item.season))];
  const [scope, setScope] = useState<ScopeKind>(
    initialEpisodes && initialEpisodes.length > 0 ? 'chosen' : 'all',
  );
  const [chosen, setChosen] = useState<Set<string>>(() => new Set(initialEpisodes ?? []));
  const [season, setSeason] = useState<number | null>(seasons[0] ?? null);
  const [from, setFrom] = useState(units[0]?.id ?? '');
  const [to, setTo] = useState(units[units.length - 1]?.id ?? '');
  const formats = FORMATS[medium];
  const remembered = useLastExportFormat(medium).data ?? null;
  const [pickedFormat, setFormat] = useState<ExportFormat | null>(null);
  const format =
    pickedFormat ?? (remembered && formats.includes(remembered) ? remembered : formats[0]);
  const [place, setPlace] = useState<Place>(
    save.location === 'folder' && save.folderUri ? 'settings' : 'downloads',
  );
  const lastFolder = useLastExportFolder().data ?? null;
  // The folder chosen here before, unless it is the Settings one.
  const picked = lastFolder && lastFolder.uri !== save.folderUri ? lastFolder : null;
  const pickFolder = usePickExportFolder();
  const start = useStartExport();
  const chooseFolder = () =>
    pickFolder.mutate(undefined, {
      onSuccess: (folder) => {
        if (folder) setPlace(folder.uri === save.folderUri ? 'settings' : 'picked');
      },
    });

  const ticked = unitsIn(units, scope, season, from, to, chosen);
  const listed = units.filter((item) => ticked.has(item.id)).map((item) => item.id);
  const scopeValue: ExportScope =
    scope === 'range'
      ? { kind: 'range', from, to }
      : scope === 'season' || scope === 'chosen'
        ? { kind: 'chosen', ids: fixed ? (initialEpisodes ?? []) : listed }
        : { kind: scope };
  const request: ExportRequest = {
    medium,
    titleIds,
    scope: scopeValue,
    format: format ?? 'cbz',
    destination: place === 'settings' || place === 'picked' ? 'folder' : place,
    folder: place === 'picked' ? picked : null,
  };
  const plan = useExportPlan(request);
  const problem = plan.data?.problem ?? (plan.isError ? plan.error.message : null);
  // Videos: what each format does with the downloaded episodes.
  const conversionOf = (value: ExportFormat) =>
    video ? plan.data?.conversions.find((conversion) => conversion.format === value) : undefined;
  const needsFfmpeg = conversionOf(format ?? 'mp4')?.needsFfmpeg ?? false;
  const ffmpeg = useExportFfmpeg(needsFfmpeg);
  const fetching = ffmpeg.install.isPending || (ffmpeg.status.data?.installing ?? false);
  const fetched = ffmpeg.status.data?.progress ?? null;

  if (start.isSuccess) {
    return (
      <Sheet title={t('export.title')} onClose={onClose}>
        <div className="flex flex-col items-center gap-3 py-2 text-center text-sm">
          <CircleCheck aria-hidden size={36} className="text-primary" />
          <p>{t('export.started', { name: start.data.label })}</p>
          <Link
            to="/downloads"
            search={{ tab: 'exports' }}
            onClick={onClose}
            className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold"
          >
            {t('export.seeProgress')}
          </Link>
        </div>
      </Sheet>
    );
  }

  const many = units.length > 1;
  const scopes: ScopeKind[] = video
    ? [
        'all',
        'downloaded',
        ...(seasons.length > 1 ? (['season'] as const) : []),
        ...(many ? (['range', 'chosen'] as const) : []),
      ]
    : ['all', 'downloaded', 'unread', ...(many ? (['range', 'chosen'] as const) : [])];
  const scopeName = (kind: ScopeKind) =>
    t(`export.scope.${kind}${video && kind !== 'unread' ? 'Video' : ''}`);
  const unitSelect = (label: string, value: string, onChange: (id: string) => void) => (
    <label className="flex min-w-0 flex-col gap-1 text-xs">
      <span className="text-muted">{label}</span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="bg-background w-full min-w-0 truncate rounded-lg px-2 py-2 text-sm"
      >
        {units.map((unit) => (
          <option key={unit.id} value={unit.id}>
            {unit.name}
          </option>
        ))}
      </select>
    </label>
  );
  const specials = t('watch.specials');
  const named = (number: number) => t('watch.season', { number });
  // Ticking a box makes the list the user's own.
  const toggle = (id: string) => {
    const next = new Set(ticked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setChosen(next);
    setScope('chosen');
  };
  const allTicked = units.length > 0 && units.every((item) => ticked.has(item.id));
  const missing = plan.data?.missing ?? 0;

  return (
    <Sheet title={t('export.title')} onClose={onClose}>
      <div className="flex flex-col gap-3 text-sm">
        {!fixed && (
          <Group label={video ? t('export.episodes') : t('export.chapters')}>
            {scopes.map((kind) => (
              <Radio
                key={kind}
                name={scopeName(kind)}
                checked={scope === kind}
                onPick={() => {
                  if (kind === 'chosen') setChosen(ticked);
                  setScope(kind);
                }}
              >
                {scopeName(kind)}
              </Radio>
            ))}
            {scope === 'season' && (
              <label className="flex min-w-0 flex-col gap-1 px-3 pb-1 text-xs">
                <span className="text-muted">{t('export.season')}</span>
                <select
                  value={season ?? ''}
                  onChange={(event) =>
                    setSeason(event.target.value === '' ? null : Number(event.target.value))
                  }
                  className="bg-background w-full rounded-lg px-2 py-2 text-sm"
                >
                  {seasons.map((value) => (
                    <option key={value ?? 'none'} value={value ?? ''}>
                      {seasonName(value, specials, named)}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {scope === 'range' && (
              <div className="flex flex-col gap-2 px-3 pb-1">
                {unitSelect(t('export.from'), from, setFrom)}
                {unitSelect(t('export.to'), to, setTo)}
              </div>
            )}
          </Group>
        )}
        {!fixed && many && (
          <div className="flex flex-col gap-1">
            <div className="flex items-center justify-between gap-2 px-1">
              <span className="text-muted text-xs">
                {t(video ? 'export.tickedEpisodes' : 'export.tickedChapters', {
                  count: ticked.size,
                })}
              </span>
              <button
                type="button"
                onClick={() => {
                  setChosen(allTicked ? new Set() : new Set(units.map((item) => item.id)));
                  setScope('chosen');
                }}
                className="text-primary rounded-full px-2 py-1 text-xs font-semibold"
              >
                {t(allTicked ? 'export.tickNone' : 'export.tickAll')}
              </button>
            </div>
            <ul
              aria-label={t(video ? 'export.pickEpisodes' : 'export.pickChapters')}
              className="bg-background -mx-1 flex max-h-56 flex-col overflow-y-auto rounded-xl p-1"
            >
              {units.map((item) => {
                const checked = ticked.has(item.id);
                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      role="checkbox"
                      aria-checked={checked}
                      onClick={() => toggle(item.id)}
                      className="hover:bg-surface flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left"
                    >
                      <span
                        aria-hidden
                        className={`grid size-5 shrink-0 place-items-center rounded border-2 ${checked ? 'border-primary bg-primary text-background' : 'border-muted'}`}
                      >
                        {checked && <Check size={14} strokeWidth={3} />}
                      </span>
                      <span className="min-w-0 flex-1 truncate">{item.name}</span>
                      {item.downloaded && (
                        <CircleCheck
                          aria-label={t(
                            video ? 'export.scope.downloadedVideo' : 'export.scope.downloaded',
                          )}
                          size={16}
                          className="text-primary shrink-0"
                        />
                      )}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        )}
        {formats.length > 1 && (
          <Group label={t('export.format')}>
            {formats.map((value) => {
              const conversion = conversionOf(value);
              return (
                <Radio
                  key={value}
                  name={t(`export.formats.${value}`)}
                  checked={format === value}
                  onPick={() => setFormat(value)}
                >
                  {t(`export.formats.${value}`)}
                  <span className="text-muted text-xs">{t(`export.formatHints.${value}`)}</span>
                  {conversion && (
                    <span
                      data-testid={`conversion-${value}`}
                      className={`text-xs ${conversion.video || conversion.audio ? 'text-primary' : 'text-muted'}`}
                    >
                      {conversionText(t, conversion)}
                    </span>
                  )}
                </Radio>
              );
            })}
          </Group>
        )}
        {needsFfmpeg && (
          <div className="bg-background flex flex-col gap-2 rounded-xl p-3 text-xs">
            <p className="text-muted">{t('export.convert.needsFfmpeg')}</p>
            <button
              type="button"
              disabled={fetching}
              onClick={() => ffmpeg.install.mutate()}
              className="bg-primary text-background self-start rounded-full px-4 py-2 font-semibold disabled:opacity-50"
            >
              {fetching
                ? fetched === null
                  ? t('export.convert.gettingFfmpeg')
                  : t('export.convert.gettingFfmpegPercent', {
                      percent: Math.floor(fetched * 100),
                    })
                : t('export.convert.getFfmpeg')}
            </button>
            {fetching && fetched !== null && (
              <div
                role="progressbar"
                aria-label={t('export.convert.gettingFfmpeg')}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.floor(fetched * 100)}
                className="bg-surface h-1.5 overflow-hidden rounded-full"
              >
                <div
                  className="bg-primary h-full origin-left transition-transform"
                  style={{ transform: `scaleX(${fetched})` }}
                />
              </div>
            )}
            {ffmpeg.install.isError && (
              <SourceIssue compact title={t('export.convert.ffmpegFailed')}>
                {ffmpeg.install.error.message}
              </SourceIssue>
            )}
          </div>
        )}
        <Group label={t('export.destination')}>
          <Radio
            name={t('export.downloads')}
            checked={place === 'downloads'}
            onPick={() => setPlace('downloads')}
          >
            {t('export.downloads')}
            <span className="text-muted text-xs">Download/Raygun</span>
          </Radio>
          {save.folderUri && (
            <Radio
              name={save.folderName ?? t('export.folder')}
              checked={place === 'settings'}
              onPick={() => setPlace('settings')}
            >
              {save.folderName ?? t('export.folder')}
            </Radio>
          )}
          {picked && (
            <Radio
              name={picked.name}
              checked={place === 'picked'}
              onPick={() => setPlace('picked')}
            >
              {picked.name}
            </Radio>
          )}
          <button
            type="button"
            disabled={pickFolder.isPending}
            onClick={chooseFolder}
            className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-2.5 text-left disabled:opacity-50"
          >
            <FolderOpen aria-hidden size={20} className="text-muted shrink-0" />
            {t('export.chooseFolder')}
          </button>
          <Radio
            name={t('export.share')}
            checked={place === 'share'}
            onPick={() => setPlace('share')}
          >
            {t('export.share')}
          </Radio>
        </Group>
        {pickFolder.isError && (
          <SourceIssue compact title={t('export.chooseFolderFailed')}>
            {pickFolder.error.message}
          </SourceIssue>
        )}
        {plan.data && !problem && (
          <p role="status" className="text-muted">
            {video
              ? t('export.planVideo', { count: plan.data.chapters })
              : `${t('export.plan', { count: plan.data.chapters })} · ${t('export.files', { count: plan.data.files })}.`}
            {missing > 0 && (
              <>
                {' '}
                {t(video ? 'export.downloadFirstVideo' : 'export.downloadFirst', {
                  count: missing,
                })}
              </>
            )}
          </p>
        )}
        {problem && (
          <p role="alert" className="text-primary">
            {problem}
          </p>
        )}
        {start.isError && (
          <p role="alert" className="text-primary">
            {start.error.message}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="rounded-full px-4 py-2.5 font-medium">
            {t('export.cancel')}
          </button>
          <button
            type="button"
            disabled={!plan.data || Boolean(problem) || needsFfmpeg || start.isPending}
            onClick={() => start.mutate(request)}
            className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
          >
            {t(video && missing > 0 ? 'export.downloadThenExport' : 'export.start')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
