import type { CatalogEntry, DetectedSource } from '@raygun/bindings';
import { Check, Link2, ListPlus, LoaderCircle, Plus, Search, Settings2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import { RepoContents } from '../extensions/RepoContents';
import { siteLabels } from '../extensions/siteLabels';
import { useLocalNetworkGate } from '../network/useLocalNetworkGate';
import { ServerSheet, ServerSignIn } from '../servers/ServerSheet';
import { AdultMark } from '../shell/AdultMark';
import { LinkOrFileField } from '../shell/LinkOrFileField';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { SHEET_FIELD, SHEET_INPUT } from '../shell/TextField';
import { AddonLogo, ProvidesChips } from '../watch/AddonRows';
import { useConfigureAddon } from '../watch/useStremio';
import { AddSeveral } from './AddSeveral';
import { withLines } from './sourceQueue';
import { useAddSource, useDetectSource } from './useSources';

/** "Looks like a Stremio addon: Cinemeta", by what was found. */
const detectedTitle = (t: TFunction, source: DetectedSource): string => {
  const name = source.name;
  switch (source.kind) {
    case 'addon':
      return t('sources.detected.addon', { name });
    case 'repository':
      return t(`sources.detected.repository.${source.format ?? 'raygun'}`, { name });
    case 'extension':
      return t(`sources.detected.extension.${source.format ?? 'raygun'}`, { name });
    case 'server':
      return t(`sources.detected.server.${source.server ?? 'jellyfin'}`, { name });
  }
};

/** One extension found on its own: its version, what it provides, what it may reach. */
const ExtensionDetails: FC<{ entry: CatalogEntry }> = ({ entry }) => {
  const { t } = useTranslation();
  const reach = entry.net.includes('*') ? t('extensions.anyWebsite') : entry.net.join(', ');
  return (
    <dl className="text-muted grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
      <dt>{t('sources.add.version')}</dt>
      <dd className="flex items-center gap-1.5">
        {entry.version}
        {entry.adult && <AdultMark />}
      </dd>
      <dt>{t('extensions.providers')}</dt>
      <dd>
        {entry.provides
          .map((kind) => t(`extensions.capability.${kind}`, { defaultValue: kind }))
          .join(', ')}
      </dd>
      <dt>{t('extensions.permissionsTitle')}</dt>
      <dd className="wrap-anywhere">{reach || t('extensions.noNetwork')}</dd>
      {entry.sites && entry.sites.length > 0 && (
        <>
          <dt>{t('extensions.runsOnTitle')}</dt>
          <dd className="wrap-anywhere">{siteLabels(entry.sites, t).join(', ')}</dd>
        </>
      )}
    </dl>
  );
};

/** What an address turned out to be, and Add (or, for a server, its sign-in). */
const DetectedCard: FC<{ source: DetectedSource; address: string; onAdded: () => void }> = ({
  source,
  address,
  onAdded,
}) => {
  const { t } = useTranslation();
  const add = useAddSource();
  const configure = useConfigureAddon();
  const title = detectedTitle(t, source);
  const addon = source.addon;
  const entry = source.kind === 'extension' ? source.entries[0] : undefined;
  const error = add.error ?? configure.error;
  return (
    <section
      aria-label={title}
      data-testid="detected-source"
      className="bg-background flex flex-col gap-2 rounded-xl px-4 py-3 text-sm"
    >
      <div className="flex items-start gap-3">
        {addon && <AddonLogo url={addon.logo} />}
        <p className="min-w-0 flex-1 font-semibold wrap-anywhere">{title}</p>
      </div>
      {source.description && (
        <p className="text-muted line-clamp-3 text-xs">{source.description}</p>
      )}
      {addon && <ProvidesChips addon={addon} />}
      {addon?.needsSetup && <p className="text-muted text-xs">{t('watch.needsSetup')}</p>}
      {source.kind === 'repository' && (
        <RepoContents
          entries={source.entries}
          label={t('extensions.contentsOf', { name: source.name })}
        />
      )}
      {entry && <ExtensionDetails entry={entry} />}
      {source.note && <p className="text-muted text-xs">{source.note}</p>}
      {(source.kind === 'repository' || source.kind === 'extension') && !source.added && (
        <p className="text-muted text-xs">{t('sources.add.trust')}</p>
      )}
      {source.kind === 'server' && source.server ? (
        <ServerSignIn kind={source.server} address={source.url} onConnected={onAdded} />
      ) : (
        <div className="flex flex-wrap justify-end gap-2">
          {addon?.configureUrl && (
            <button
              type="button"
              onClick={() => configure.mutate(addon.url)}
              className="bg-surface flex items-center gap-1.5 rounded-full px-4 py-2 text-sm"
            >
              <Settings2 aria-hidden size={16} />
              {t('watch.setUp')}
            </button>
          )}
          {!addon?.needsSetup && (
            <button
              type="button"
              disabled={source.added || add.isPending}
              onClick={() => add.mutate(address, { onSuccess: onAdded })}
              className="bg-primary text-background flex items-center gap-1.5 rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
            >
              {add.isPending ? (
                <LoaderCircle aria-hidden size={16} className="animate-spin" />
              ) : source.added ? (
                <Check aria-hidden size={16} />
              ) : (
                <Plus aria-hidden size={16} />
              )}
              {source.added
                ? t(source.kind === 'extension' ? 'sources.add.installed' : 'sources.add.added')
                : t('sources.add.add')}
            </button>
          )}
        </div>
      )}
      {error && <SourceIssue compact>{error.message}</SourceIssue>}
    </section>
  );
};

type AddMode = 'one' | 'several';
const MODES: AddMode[] = ['one', 'several'];

/** Pasted text holding more than one address. */
const severalIn = (text: string) =>
  text.split(/\r?\n/).filter((line) => line.trim() && !line.trim().startsWith('#')).length > 1;

/** One address: looked at, then shown with Add (or a server's sign-in). */
const OneAddress: FC<{
  address: string;
  onAddressChange: (address: string) => void;
  /** A list was pasted into the field. */
  onList: (pasted: string) => void;
  onByHand: () => void;
  onClose: () => void;
  detect: ReturnType<typeof useDetectSource>;
}> = ({ address, onAddressChange, onList, onByHand, onClose, detect }) => {
  const { t } = useTranslation();
  const gate = useLocalNetworkGate('source');
  const typed = address.trim();
  // What's shown is about what's typed: typing again asks again.
  const current = detect.data && detect.variables === typed ? detect.data : null;
  const failed = detect.error && detect.variables === typed ? detect.error : null;
  const looking = detect.isPending || gate.checking;

  const look = (value: string) => {
    const wanted = value.trim();
    // An address on the home network needs Android 17's local network access first.
    if (wanted) void gate.guard(wanted, () => detect.mutate(wanted));
  };

  return (
    <>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          look(address);
        }}
        className="flex flex-col gap-3 text-sm"
      >
        <LinkOrFileField
          pick="both"
          type="text"
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          autoFocus
          value={address}
          onChange={onAddressChange}
          onPicked={look}
          onPaste={(event) => {
            const pasted =
              event.clipboardData.getData('text/plain') || event.clipboardData.getData('text');
            if (!severalIn(pasted)) return;
            event.preventDefault();
            onList(pasted);
          }}
          placeholder={t('sources.add.placeholder')}
          aria-label={t('sources.add.address')}
          className={SHEET_FIELD}
          inputClassName={SHEET_INPUT}
        />
        <p className="text-muted text-xs">{t('sources.add.hint')}</p>
        {looking && (
          <p role="status" className="text-muted flex items-center gap-2 text-xs">
            <LoaderCircle aria-hidden size={14} className="animate-spin" />
            {t('sources.add.looking')}
          </p>
        )}
        {failed && <SourceIssue compact>{failed.message}</SourceIssue>}
        {!current && (
          <div className="flex flex-wrap items-center justify-end gap-2">
            {!looking && (
              <button
                type="button"
                onClick={onByHand}
                className="text-primary rounded-full px-3 py-2 text-sm font-medium"
              >
                {t('sources.add.byHand')}
              </button>
            )}
            <button
              type="submit"
              disabled={!typed || looking}
              className="bg-primary text-background flex items-center gap-1.5 rounded-full px-5 py-2.5 font-semibold disabled:opacity-60"
            >
              {looking ? (
                <LoaderCircle aria-hidden size={16} className="animate-spin" />
              ) : (
                <Search aria-hidden size={16} />
              )}
              {t('sources.add.lookUp')}
            </button>
          </div>
        )}
      </form>
      {current && <DetectedCard source={current} address={typed} onAdded={onClose} />}
      {gate.sheet}
    </>
  );
};

/**
 * Sources → Add (M46.52): one field for any address. Look up asks Rust what it is (a Stremio
 * addon, a repository of any format Raygun reads, one extension, a server) and shows it before
 * it's added; a file or a folder picked beside the field is looked up at once. A server can also
 * be connected by hand (its kind picked), before or after a look up. Several takes a list, one
 * address a line, added one after another; a list pasted into the one address's field goes
 * there.
 */
export const AddSourceSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const [mode, setMode] = useState<AddMode>('one');
  const [address, setAddress] = useState('');
  const [list, setList] = useState('');
  const [byHand, setByHand] = useState(false);
  // Kept here, so what was found stays while the other mode shows.
  const detect = useDetectSource();

  if (byHand) return <ServerSheet onClose={onClose} />;

  return (
    <Sheet title={t('sources.add.title')} onClose={onClose}>
      <div role="tablist" aria-label={t('sources.add.modes')} className="flex gap-2">
        {MODES.map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={mode === id}
            onClick={() => setMode(id)}
            className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm ${mode === id ? 'bg-primary text-background font-semibold' : 'bg-background'}`}
          >
            {id === 'one' ? <Link2 aria-hidden size={14} /> : <ListPlus aria-hidden size={14} />}
            {t(`sources.add.mode.${id}`)}
          </button>
        ))}
      </div>
      {mode === 'several' ? (
        <AddSeveral text={list} onTextChange={setList} />
      ) : (
        <OneAddress
          address={address}
          onAddressChange={setAddress}
          onList={(pasted) => {
            setList((before) => withLines(before, [pasted.replace(/\r\n?/g, '\n').trimEnd()]));
            setMode('several');
          }}
          onByHand={() => setByHand(true)}
          onClose={onClose}
          detect={detect}
        />
      )}
    </Sheet>
  );
};
