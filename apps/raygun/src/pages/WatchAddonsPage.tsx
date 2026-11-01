import type { StremioAddon } from '@raygun/bindings';
import { Check, LoaderCircle, Plus, Puzzle, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { Sheet } from '../shell/Sheet';
import { TopBarAction } from '../shell/TopBar';
import {
  useAddAddon,
  useRemoveAddon,
  useStremioAddons,
  useStremioSuggestions,
} from '../watch/useStremio';
import { PageHeader } from './PageHeader';

const AddonLogo: FC<{ url: string | null }> = ({ url }) =>
  url ? (
    <img src={url} alt="" loading="lazy" className="size-10 shrink-0 rounded-lg object-contain" />
  ) : (
    <span
      aria-hidden
      className="bg-background text-muted grid size-10 shrink-0 place-items-center rounded-lg"
    >
      <Puzzle size={20} />
    </span>
  );

const AddonItem: FC<{ addon: StremioAddon }> = ({ addon }) => {
  const { t } = useTranslation();
  const remove = useRemoveAddon();
  const serves = addon.resources.map((resource) =>
    t(`watch.resource.${resource}`, { defaultValue: resource }),
  );
  return (
    <li aria-label={addon.name} className="bg-surface flex items-start gap-3 rounded-xl px-4 py-3">
      <AddonLogo url={addon.logo} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium wrap-anywhere">
          {addon.name}
          {addon.version && <span className="text-muted text-xs"> {addon.version}</span>}
        </span>
        {addon.error ? (
          <span className="text-primary text-xs wrap-anywhere">
            {t('watch.addonFailed', { message: addon.error })}
          </span>
        ) : (
          <>
            {addon.description && (
              <span className="text-muted line-clamp-2 text-xs">{addon.description}</span>
            )}
            {serves.length > 0 && (
              <span className="text-muted text-xs">
                {t('watch.addonServes', { list: serves.join(', ') })}
              </span>
            )}
          </>
        )}
      </div>
      <button
        type="button"
        aria-label={t('watch.removeAddon', { name: addon.name })}
        disabled={remove.isPending}
        onClick={() => remove.mutate(addon.id)}
        className="text-muted grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-50"
      >
        <Trash2 aria-hidden size={18} />
      </button>
    </li>
  );
};

/** Stremio addons: what's installed, well-known ones, and "+" to add one by its address. */
export const WatchAddonsPage: FC = () => {
  const { t } = useTranslation();
  const addons = useStremioAddons();
  const suggestions = useStremioSuggestions();
  const add = useAddAddon();
  const localNetwork = useLocalNetworkGate('addon');
  const [url, setUrl] = useState('');
  const [addingByUrl, setAddingByUrl] = useState(false);
  const missing = (suggestions.data ?? []).filter((suggestion) => !suggestion.added);

  const onAdd = (event: FormEvent) => {
    event.preventDefault();
    const address = url.trim();
    if (address) {
      // An addon on the user's network needs Android 17's local network access first.
      void localNetwork.guard(address, () =>
        add.mutate(address, {
          onSuccess: () => {
            setUrl('');
            setAddingByUrl(false);
          },
        }),
      );
    }
  };
  const adding = add.isPending || localNetwork.checking;

  return (
    <section data-testid="watch-addons-page" className="flex flex-col gap-5">
      <PageHeader
        back
        fallback="/browse"
        title={t('watch.addons')}
        actions={
          <TopBarAction
            label={t('watch.addAddon')}
            icon={Plus}
            onClick={() => setAddingByUrl(true)}
          />
        }
      />
      <p className="text-muted text-sm">{t('watch.addonsHint')}</p>
      {add.error && !addingByUrl && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {add.error.message}
        </p>
      )}
      {addons.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {addons.error.message}
        </p>
      )}
      {addons.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {addons.data && addons.data.length === 0 && (
        <p className="text-muted text-sm">{t('watch.noAddons')}</p>
      )}
      {addons.data && addons.data.length > 0 && (
        <ul aria-label={t('watch.installedAddons')} className="flex flex-col gap-2">
          {addons.data.map((addon) => (
            <AddonItem key={addon.id} addon={addon} />
          ))}
        </ul>
      )}
      {missing.length > 0 && (
        <section aria-labelledby="addon-suggestions" className="flex flex-col gap-2">
          <h2 id="addon-suggestions" className="font-semibold">
            {t('watch.suggestedAddons')}
          </h2>
          <ul className="flex flex-col gap-2">
            {missing.map((suggestion) => (
              <li
                key={suggestion.url}
                aria-label={suggestion.name}
                className="bg-surface flex items-center gap-3 rounded-xl px-4 py-3"
              >
                <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="font-medium">{suggestion.name}</span>
                  <span className="text-muted text-xs">{suggestion.description}</span>
                </div>
                <button
                  type="button"
                  aria-label={t('watch.addNamed', { name: suggestion.name })}
                  disabled={add.isPending}
                  onClick={() => add.mutate(suggestion.url)}
                  className="bg-primary text-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60"
                >
                  {suggestion.added ? (
                    <Check aria-hidden size={16} />
                  ) : (
                    <Plus aria-hidden size={16} />
                  )}
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {addingByUrl && (
        <Sheet title={t('watch.addAddon')} onClose={() => setAddingByUrl(false)}>
          <form onSubmit={onAdd} className="flex flex-col gap-3 text-sm">
            <input
              type="text"
              inputMode="url"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              autoFocus
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://…/manifest.json"
              aria-label={t('watch.addAddon')}
              className="bg-background placeholder:text-muted rounded-xl px-4 py-3 outline-none"
            />
            {add.error && (
              <p role="alert" className="text-primary text-xs wrap-anywhere">
                {add.error.message}
              </p>
            )}
            <button
              type="submit"
              disabled={!url.trim() || adding}
              className="bg-primary text-background flex items-center gap-1.5 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
            >
              {adding ? (
                <LoaderCircle aria-hidden size={16} className="animate-spin" />
              ) : (
                <Plus aria-hidden size={16} />
              )}
              {t('watch.add')}
            </button>
          </form>
        </Sheet>
      )}
      {localNetwork.sheet}
    </section>
  );
};
