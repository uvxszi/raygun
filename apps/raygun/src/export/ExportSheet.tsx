import type { ExportDestination, ExportFormat, ExportRequest, ExportScope } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { CircleCheck } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from '../manga/medium';
import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { withSave } from './saveDefaults';
import { useExportPlan, useStartExport } from './useExports';

const FORMATS: Record<Medium, ExportFormat[]> = {
  manga: ['cbz', 'pdf', 'pdfPerChapter', 'images'],
  novel: ['epub', 'text'],
  video: ['video'],
};

type ScopeKind = 'all' | 'downloaded' | 'unread' | 'range';

/** A chapter for the range pickers, in reading order. */
export type ExportChapter = { id: string; name: string };

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

/**
 * Export a title (or a selection) as files to keep (M46.1): which chapters, in what format, and
 * where. It says first how many chapters must download, which then go through the normal queue.
 */
export const ExportSheet: FC<{
  medium: Medium;
  titleIds: string[];
  /** One title's chapters in reading order, for exporting a range. */
  chapters?: ExportChapter[];
  onClose: () => void;
}> = ({ medium, titleIds, chapters = [], onClose }) => {
  const { t } = useTranslation();
  const save = withSave(useSettings().data?.save);
  const video = medium === 'video';
  const [scope, setScope] = useState<ScopeKind>(video ? 'downloaded' : 'all');
  const [from, setFrom] = useState(chapters[0]?.id ?? '');
  const [to, setTo] = useState(chapters[chapters.length - 1]?.id ?? '');
  const [format, setFormat] = useState<ExportFormat>(FORMATS[medium][0] ?? 'cbz');
  const [destination, setDestination] = useState<ExportDestination>(
    save.location === 'folder' && save.folderUri ? 'folder' : 'downloads',
  );
  const start = useStartExport();

  const scopeValue: ExportScope = scope === 'range' ? { kind: 'range', from, to } : { kind: scope };
  const request: ExportRequest = {
    medium,
    titleIds,
    scope: scopeValue,
    format,
    destination,
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
    ? ['downloaded']
    : ['all', 'downloaded', 'unread', ...(chapters.length > 1 ? (['range'] as const) : [])];
  const chapterSelect = (label: string, value: string, onChange: (id: string) => void) => (
    <label className="flex min-w-0 flex-col gap-1 text-xs">
      <span className="text-muted">{label}</span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="bg-background w-full min-w-0 truncate rounded-lg px-2 py-2 text-sm"
      >
        {chapters.map((chapter) => (
          <option key={chapter.id} value={chapter.id}>
            {chapter.name}
          </option>
        ))}
      </select>
    </label>
  );

  return (
    <Sheet title={t('export.title')} onClose={onClose}>
      <div className="flex flex-col gap-3 text-sm">
        <Group label={video ? t('export.episodes') : t('export.chapters')}>
          {scopes.map((kind) => (
            <Radio
              key={kind}
              name={t(`export.scope.${kind}${video ? 'Video' : ''}`)}
              checked={scope === kind}
              onPick={() => setScope(kind)}
            >
              {t(`export.scope.${kind}${video ? 'Video' : ''}`)}
            </Radio>
          ))}
          {scope === 'range' && (
            <div className="flex flex-col gap-2 px-3 pb-1">
              {chapterSelect(t('export.from'), from, setFrom)}
              {chapterSelect(t('export.to'), to, setTo)}
            </div>
          )}
        </Group>
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
            checked={destination === 'downloads'}
            onPick={() => setDestination('downloads')}
          >
            {t('export.downloads')}
            <span className="text-muted text-xs">Download/Raygun</span>
          </Radio>
          <Radio
            name={save.folderName ?? t('export.folder')}
            checked={destination === 'folder'}
            disabled={!save.folderUri}
            onPick={() => setDestination('folder')}
          >
            {save.folderName ?? t('export.folder')}
            {!save.folderUri && (
              <span className="text-muted text-xs">{t('export.folderHint')}</span>
            )}
          </Radio>
          <Radio
            name={t('export.share')}
            checked={destination === 'share'}
            onPick={() => setDestination('share')}
          >
            {t('export.share')}
          </Radio>
        </Group>
        {plan.data && !problem && (
          <p role="status" className="text-muted">
            {video
              ? t('export.planVideo', { count: plan.data.chapters })
              : `${t('export.plan', { count: plan.data.chapters })} · ${t('export.files', { count: plan.data.files })}.`}
            {plan.data.missing > 0 && (
              <> {t('export.downloadFirst', { count: plan.data.missing })}</>
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
            {t('export.start')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};
