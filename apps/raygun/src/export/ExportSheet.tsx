import type { ExportFormat, ExportRequest, ExportScope } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Check, CircleCheck, FolderOpen } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from '../manga/medium';
import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { withSave } from './saveDefaults';
import { SourceIssue } from '../shell/SourceIssue';
import {
  useExportPlan,
  useLastExportFolder,
  usePickExportFolder,
  useStartExport,
} from './useExports';

const FORMATS: Record<Medium, ExportFormat[]> = {
  manga: ['cbz', 'pdf', 'pdfPerChapter', 'images'],
  novel: ['epub', 'text'],
  video: ['video'],
};

/** `season` and `chosen` go to Rust as a list of episodes. */
type ScopeKind = 'all' | 'downloaded' | 'unread' | 'range' | 'season' | 'chosen';

/** Where the files go: the Settings folder, or the one last chosen here (`picked`). */
type Place = 'downloads' | 'settings' | 'picked' | 'share';

/** A chapter for the range pickers, in reading order. */
export type ExportChapter = { id: string; name: string };

/** An episode for the range pickers and its box, in watching order (ids: stored or the source's). */
export type ExportEpisode = {
  id: string;
  name: string;
  season: number | null;
  downloaded: boolean;
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

/** The episodes a scope takes, as the sheet ticks them. */
const episodesIn = (
  episodes: ExportEpisode[],
  scope: ScopeKind,
  season: number | null,
  from: string,
  to: string,
  chosen: Set<string>,
): Set<string> => {
  switch (scope) {
    case 'downloaded':
      return new Set(episodes.filter((item) => item.downloaded).map((item) => item.id));
    case 'season':
      return new Set(episodes.filter((item) => item.season === season).map((item) => item.id));
    case 'range': {
      const start = episodes.findIndex((item) => item.id === from);
      const end = episodes.findIndex((item) => item.id === to);
      if (start < 0 || end < 0) return new Set();
      return new Set(
        episodes.slice(Math.min(start, end), Math.max(start, end) + 1).map((item) => item.id),
      );
    }
    case 'chosen':
      return chosen;
    default:
      return new Set(episodes.map((item) => item.id));
  }
};

/**
 * Export a title (or a selection) as files to keep (M46.1): which chapters or episodes, in what
 * format, and where. It says first how many must download, which then go through the normal
 * queue (episodes find their own links).
 */
export const ExportSheet: FC<{
  medium: Medium;
  titleIds: string[];
  /** One title's chapters in reading order, for exporting a range. */
  chapters?: ExportChapter[];
  /** One title's episodes in watching order: seasons, ranges and boxes per episode. */
  episodes?: ExportEpisode[];
  /** Episodes ticked when the sheet opens (a selection on the title page). */
  initialEpisodes?: string[];
  onClose: () => void;
}> = ({ medium, titleIds, chapters = [], episodes = [], initialEpisodes, onClose }) => {
  const { t } = useTranslation();
  const save = withSave(useSettings().data?.save);
  const video = medium === 'video';
  const seasons = useMemo(() => [...new Set(episodes.map((item) => item.season))], [episodes]);
  const [scope, setScope] = useState<ScopeKind>(
    initialEpisodes && initialEpisodes.length > 0 ? 'chosen' : 'all',
  );
  const [chosen, setChosen] = useState<Set<string>>(() => new Set(initialEpisodes ?? []));
  const [season, setSeason] = useState<number | null>(seasons[0] ?? null);
  const units = video ? episodes : chapters;
  const [from, setFrom] = useState(units[0]?.id ?? '');
  const [to, setTo] = useState(units[units.length - 1]?.id ?? '');
  const [format, setFormat] = useState<ExportFormat>(FORMATS[medium][0] ?? 'cbz');
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

  const ticked = episodesIn(episodes, scope, season, from, to, chosen);
  const scopeValue: ExportScope =
    scope === 'range'
      ? { kind: 'range', from, to }
      : scope === 'season' || scope === 'chosen'
        ? {
            kind: 'chosen',
            ids: episodes.filter((item) => ticked.has(item.id)).map((item) => item.id),
          }
        : { kind: scope };
  const request: ExportRequest = {
    medium,
    titleIds,
    scope: scopeValue,
    format,
    destination: place === 'settings' || place === 'picked' ? 'folder' : place,
    folder: place === 'picked' ? picked : null,
  };
  const plan = useExportPlan(request);
  const problem = plan.data?.problem ?? (plan.isError ? plan.error.message : null);

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

  const scopes: ScopeKind[] = video
    ? [
        'all',
        'downloaded',
        ...(seasons.length > 1 ? (['season'] as const) : []),
        ...(episodes.length > 1 ? (['range', 'chosen'] as const) : []),
      ]
    : ['all', 'downloaded', 'unread', ...(chapters.length > 1 ? (['range'] as const) : [])];
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
  const missing = plan.data?.missing ?? 0;

  return (
    <Sheet title={t('export.title')} onClose={onClose}>
      <div className="flex flex-col gap-3 text-sm">
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
        {video && episodes.length > 1 && (
          <ul
            aria-label={t('export.pickEpisodes')}
            className="bg-background -mx-1 flex max-h-56 flex-col overflow-y-auto rounded-xl p-1"
          >
            {episodes.map((item) => {
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
                        aria-label={t('export.scope.downloadedVideo')}
                        size={16}
                        className="text-primary shrink-0"
                      />
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
        {FORMATS[medium].length > 1 && (
          <Group label={t('export.format')}>
            {FORMATS[medium].map((value) => (
              <Radio
                key={value}
                name={t(`export.formats.${value}`)}
                checked={format === value}
                onPick={() => setFormat(value)}
              >
                {t(`export.formats.${value}`)}
                <span className="text-muted text-xs">{t(`export.formatHints.${value}`)}</span>
              </Radio>
            ))}
          </Group>
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
            disabled={!plan.data || Boolean(problem) || start.isPending}
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
