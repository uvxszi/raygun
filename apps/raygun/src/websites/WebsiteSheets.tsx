import type { Website } from '@raygun/bindings';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT, TextField } from '../shell/TextField';
import { showToast } from '../shell/toastStore';
import { useAddWebsite, useEditWebsite, useRemoveWebsite } from './useWebsites';

/** Add a site, or change one: its address and an optional name. */
export const WebsiteSheet: FC<{ site?: Website; onClose: () => void }> = ({ site, onClose }) => {
  const { t } = useTranslation();
  const add = useAddWebsite();
  const edit = useEditWebsite();
  const [url, setUrl] = useState(site?.url ?? '');
  const [name, setName] = useState(site?.name ?? '');
  const saving = add.isPending || edit.isPending;
  const error = add.error ?? edit.error;

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (site) {
      edit.mutate({ id: site.id, name: name.trim(), url: url.trim() }, { onSuccess: onClose });
      return;
    }
    add.mutate(
      { name: name.trim() || null, url: url.trim() },
      {
        onSuccess: (added) => {
          // Installed site scripts that run on the new site say so.
          if (added.scripts?.length) {
            showToast(t('websites.scriptsAdded', { names: added.scripts.join(', ') }));
          }
          onClose();
        },
      },
    );
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
export const RemoveWebsiteSheet: FC<{ site: Website; onClose: () => void }> = ({
  site,
  onClose,
}) => {
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
