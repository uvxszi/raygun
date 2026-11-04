import type { Website } from '@raygun/bindings';
import { Pencil, Plus, RotateCcw, ShieldCheck, ShieldOff, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { LetterAvatar } from '../shell/LetterAvatar';
import { OverflowMenu } from '../shell/OverflowMenu';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { matchesSearch } from '../shell/useListSearch';
import {
  useAddWebsite,
  useEditWebsite,
  useOpenWebsite,
  useRemoveWebsite,
  useResetWebsite,
  useSetWebsiteBlocking,
  useWebsites,
} from './useWebsites';

/** Add a site, or change one: its address and an optional name. */
const WebsiteSheet: FC<{ site?: Website; onClose: () => void }> = ({ site, onClose }) => {
  const { t } = useTranslation();
  const add = useAddWebsite();
  const edit = useEditWebsite();
  const [url, setUrl] = useState(site?.url ?? '');
  const [name, setName] = useState(site?.name ?? '');
  const saving = add.isPending || edit.isPending;
  const error = add.error ?? edit.error;

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    const done = { onSuccess: onClose };
    if (site) edit.mutate({ id: site.id, name: name.trim(), url: url.trim() }, done);
    else add.mutate({ name: name.trim() || null, url: url.trim() }, done);
  };

  return (
    <Sheet title={site ? t('websites.edit') : t('websites.add')} onClose={onClose}>
      <form onSubmit={onSubmit} className="flex flex-col gap-3 text-sm">
        <TextField
          type="text"
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          autoFocus
          value={url}
          onChange={setUrl}
          placeholder="example.com"
          aria-label={t('websites.address')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
        />
        <TextField
          type="text"
          value={name}
          onChange={setName}
          placeholder={t('websites.namePlaceholder')}
          aria-label={t('websites.name')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
        />
        {error && (
          // Rust says what's wrong with the address in plain words.
          <p role="alert" className="text-primary text-xs wrap-anywhere">
            {error.message}
          </p>
        )}
        <button
          type="submit"
          disabled={!url.trim() || saving}
          className="bg-primary text-background flex items-center gap-1.5 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
        >
          {site ? t('websites.save') : t('websites.addButton')}
        </button>
      </form>
    </Sheet>
  );
};

/** Asks before a site leaves the list (it can be added again, but its place is forgotten). */
const RemoveSheet: FC<{ site: Website; onClose: () => void }> = ({ site, onClose }) => {
  const { t } = useTranslation();
  const remove = useRemoveWebsite();
  return (
    <Sheet title={t('websites.removeTitle', { name: site.name })} onClose={onClose}>
      <div className="flex flex-col gap-3 text-sm">
        {remove.error && (
          <SourceIssue compact title={t('websites.removeFailed')}>
            {remove.error.message}
          </SourceIssue>
        )}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="hover:bg-background rounded-full px-4 py-2.5 font-medium"
          >
            {t('websites.cancel')}
          </button>
          <button
            type="button"
            disabled={remove.isPending}
            onClick={() => remove.mutate(site.id, { onSuccess: onClose })}
            className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
          >
            {t('websites.removeConfirm')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};

/**
 * Browse → Sources → Websites (M46.3): sites the user added, opened in Raygun's built-in
 * browser with its ad and popup blocker.
 */
export const WebsitesSection: FC<{ query?: string }> = ({ query = '' }) => {
  const { t } = useTranslation();
  const websites = useWebsites();
  const shown = websites.data?.filter((site) => matchesSearch(query, [site.name, site.url]));
  const open = useOpenWebsite();
  const reset = useResetWebsite();
  const blocking = useSetWebsiteBlocking();
  const [adding, setAdding] = useState(false);
  const [editing, setEditing] = useState<Website | null>(null);
  const [removing, setRemoving] = useState<Website | null>(null);
  const title = t('websites.title');
  const opened = websites.data?.find((site) => site.id === open.variables);

  return (
    <section aria-label={title} className="flex flex-col">
      <div className="flex items-center justify-between gap-2 px-1 pt-2 pb-1">
        <h2 className="text-muted text-sm font-medium">{title}</h2>
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="text-primary flex items-center gap-1 rounded-full px-3 py-1.5 text-sm font-medium"
        >
          <Plus aria-hidden size={16} />
          {t('websites.add')}
        </button>
      </div>
      {websites.error && (
        <SourceIssue compact title={t('websites.loadFailed')}>
          {websites.error.message}
        </SourceIssue>
      )}
      {websites.data?.length === 0 && (
        <p className="text-muted px-1 pb-2 text-sm">{t('websites.empty')}</p>
      )}
      <ul className="flex flex-col">
        {shown?.map((site) => (
          <li key={site.id} className="flex items-center gap-1">
            <button
              type="button"
              aria-label={t('websites.open', { name: site.name })}
              onClick={() => open.mutate(site.id)}
              className="hover:bg-surface flex min-w-0 flex-1 items-center gap-4 rounded-xl px-1 py-2.5 text-left"
            >
              <LetterAvatar name={site.name} logo={site.icon} />
              <span className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{site.name}</span>
                <span className="text-muted truncate text-xs">
                  {[site.host, site.blockAds ? null : t('websites.adsAllowed')]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </span>
            </button>
            <OverflowMenu
              label={t('websites.menu', { name: site.name })}
              items={[
                { label: t('websites.edit'), icon: Pencil, onSelect: () => setEditing(site) },
                site.blockAds
                  ? {
                      label: t('websites.allowHere'),
                      icon: ShieldOff,
                      onSelect: () => blocking.mutate({ id: site.id, on: false }),
                    }
                  : {
                      label: t('websites.blockHere'),
                      icon: ShieldCheck,
                      onSelect: () => blocking.mutate({ id: site.id, on: true }),
                    },
                ...(site.lastUrl
                  ? [
                      {
                        label: t('websites.startOver'),
                        icon: RotateCcw,
                        onSelect: () => reset.mutate(site.id),
                      },
                    ]
                  : []),
                {
                  label: t('websites.remove'),
                  icon: Trash2,
                  onSelect: () => setRemoving(site),
                },
              ]}
            />
          </li>
        ))}
      </ul>
      {open.error && (
        <SourceIssue
          compact
          title={t('websites.openFailed', { name: opened?.name ?? t('websites.title') })}
        >
          {open.error.message}
        </SourceIssue>
      )}
      {(reset.error ?? blocking.error) && (
        <SourceIssue compact title={t('websites.saveFailed')}>
          {(reset.error ?? blocking.error)?.message}
        </SourceIssue>
      )}
      {adding && <WebsiteSheet onClose={() => setAdding(false)} />}
      {editing && <WebsiteSheet site={editing} onClose={() => setEditing(null)} />}
      {removing && <RemoveSheet site={removing} onClose={() => setRemoving(null)} />}
    </section>
  );
};
