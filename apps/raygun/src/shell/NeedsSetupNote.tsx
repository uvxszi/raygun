import type { SourceSetup } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { ChevronDown, KeyRound } from 'lucide-react';
import { useId, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSourcesSetup } from '../sources/useSourcesSetup';

/** A source named by an answer that said it needs setup (its error text), before Rust's list has it. */
export type SetupSource = { extensionId: string; name: string };

type NeedsSetupNoteProps = {
  /**
   * The provider kinds this place asks (`lyrics`, `music.metadata`...): every source of those
   * kinds that waits for a key or sign-in is counted.
   */
  kinds?: readonly string[];
  /** Sources whose answers here said they need setup. */
  sources?: readonly SetupSource[];
  className?: string;
};

/** A source said it needs setup, but Rust's list doesn't have it yet. */
const fromAnswer = ({ extensionId, name }: SetupSource): SourceSetup => ({
  extensionId,
  name,
  kinds: [],
  need: { reason: 'refused', missing: [], detail: null },
  server: false,
});

/**
 * The sources that need a key or sign-in, in one line instead of an error each: "3 sources need a
 * key or sign-in", collapsed. Opened, it lists them, each leading to where it's set up (an
 * extension's settings, a server's sign-in in Sources). Raygun asks them nothing meanwhile, so
 * they're neither failures nor counted as failing. Nothing shows when none waits.
 */
export const NeedsSetupNote: FC<NeedsSetupNoteProps> = ({
  kinds = [],
  sources = [],
  className = '',
}) => {
  const { t } = useTranslation();
  const listed = useSourcesSetup();
  const [open, setOpen] = useState(false);
  const listId = useId();
  const waiting = new Map<string, SourceSetup>();
  for (const source of listed) {
    if (source.kinds.some((kind) => kinds.includes(kind))) waiting.set(source.extensionId, source);
  }
  for (const named of sources) {
    if (waiting.has(named.extensionId)) continue;
    const known = listed.find((source) => source.extensionId === named.extensionId);
    waiting.set(named.extensionId, known ?? fromAnswer(named));
  }
  if (waiting.size === 0) return null;
  const shown = [...waiting.values()].sort((a, b) => a.name.localeCompare(b.name));

  const what = (source: SourceSetup) => {
    if (source.need.reason === 'signIn') return t('sourcesSetup.signIn');
    if (source.need.reason === 'refused') return t('sourcesSetup.refused');
    return t('sourcesSetup.missing', { fields: source.need.missing.join(', ') });
  };

  return (
    <div
      data-testid="needs-setup"
      className={`bg-surface/70 text-muted flex flex-col gap-1 rounded-xl px-3 py-2 text-sm ${className}`}
    >
      <button
        type="button"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen(!open)}
        className="hover:text-foreground flex items-center gap-2 text-left"
      >
        <KeyRound aria-hidden size={16} className="shrink-0" />
        <span className="text-foreground min-w-0 flex-1 font-medium">
          {t('sourcesSetup.count', { count: shown.length })}
        </span>
        <ChevronDown
          aria-hidden
          size={16}
          className={`shrink-0 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>
      <ul id={listId} hidden={!open} className="flex flex-col">
        {shown.map((source) => {
          const row = (
            <>
              <span className="text-foreground min-w-0 flex-1 truncate">{source.name}</span>
              <span className="min-w-0 truncate text-xs">{what(source)}</span>
              <span className="text-foreground shrink-0 text-xs font-semibold">
                {t('sourcesSetup.setUp')}
              </span>
            </>
          );
          const style = 'hover:bg-background flex items-center gap-2 rounded-lg px-2 py-1.5';
          return (
            <li key={source.extensionId}>
              {source.server ? (
                <Link to="/sources" search={{ type: 'servers' }} className={style}>
                  {row}
                </Link>
              ) : (
                <Link to="/extensions/detail" search={{ id: source.extensionId }} className={style}>
                  {row}
                </Link>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
};
