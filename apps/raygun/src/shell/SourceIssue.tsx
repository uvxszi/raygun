import { Check, CloudOff, Copy, RotateCw } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { issueKind, readIssue } from '../lib/issue';
import { isOfflineError } from '../network/useNetworkMode';
import { OfflineNote } from '../network/OfflineNote';
import { setupSourceOf } from '../sources/useSourcesSetup';
import { BrowserCheckAction } from './BrowserCheckAction';
import { NeedsSetupNote } from './NeedsSetupNote';

type SourceIssueProps = {
  /**
   * The error. As text (what a command failed with), its first line, or the kind it names, is
   * the headline and the raw rest goes under "More info"; other content goes under More info as
   * it is.
   */
  children: ReactNode;
  /** The source's name, when known: "MangaDex didn't answer in time". */
  source?: string | null;
  /** What failed, when it isn't a source: "Couldn't add the folder". */
  title?: string;
  /** Asks the source again. */
  onRetry?: () => void;
  /** One line, for rows and small spaces. */
  compact?: boolean;
  testId?: string;
  className?: string;
};

/**
 * How a source failing looks (M46.17): calm, never a red error. Sources fail all the time (sites
 * change, block, time out), so the page says which one had an issue and what happened, in words
 * a listener understands (M46.48); "More info" shows the full error, to read or copy.
 */
export const SourceIssue: FC<SourceIssueProps> = ({
  children,
  source,
  title,
  onRetry,
  compact = false,
  testId,
  className = '',
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const [details, setDetails] = useState<HTMLDivElement | null>(null);
  // Offline Mode isn't a source's failure: it says so, calmly, with no retry.
  if (isOfflineError(children)) {
    return <OfflineNote compact={compact} testId={testId} className={className} />;
  }
  // Neither is a source waiting for its key or sign-in: it says where to set it up.
  const waiting = setupSourceOf(children, source);
  if (waiting) {
    return <NeedsSetupNote sources={[waiting]} className={className} />;
  }
  const issue =
    typeof children === 'string' && children.trim() ? readIssue(children, t, source) : null;
  // A browser check may be passed by hand when that's turned on (experimental).
  const browserCheck =
    typeof children === 'string' && issueKind(children) === 'bot-check' ? children : null;
  const headline =
    title ?? issue?.summary ?? (source ? t('issue.withSource', { source }) : t('issue.title'));
  // A given title says what failed; then the whole error is the detail.
  const more: ReactNode = issue && !title ? issue.details : children;

  const copy = () => {
    const text = details?.textContent?.trim();
    if (!text) return;
    void navigator.clipboard
      ?.writeText(text)
      .then(() => setCopied(true))
      .catch(() => undefined);
  };

  return (
    <div
      role="alert"
      data-testid={testId ?? 'source-issue'}
      className={`bg-surface/70 text-muted flex flex-col gap-1.5 rounded-xl text-sm ${
        compact ? 'px-3 py-2' : 'px-4 py-3'
      } ${className}`}
    >
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
        <CloudOff aria-hidden size={16} className="shrink-0" />
        <span className="text-foreground min-w-0 font-medium wrap-anywhere">{headline}</span>
        <span className="ml-auto flex items-center gap-1">
          {onRetry && (
            <button
              type="button"
              onClick={onRetry}
              className="hover:text-foreground hover:bg-background flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-semibold"
            >
              <RotateCw aria-hidden size={12} />
              {t('issue.retry')}
            </button>
          )}
          {more && (
            <button
              type="button"
              aria-expanded={open}
              onClick={() => setOpen(!open)}
              className="hover:text-foreground hover:bg-background rounded-full px-2.5 py-1 text-xs font-semibold"
            >
              {open ? t('issue.less') : t('issue.more')}
            </button>
          )}
        </span>
      </div>
      {browserCheck && (
        <BrowserCheckAction error={browserCheck} source={source} onRetry={onRetry} />
      )}
      {/* Content that isn't an error text (older callers) keeps the general line. */}
      {!compact && !title && !issue && <p>{t('issue.calm')}</p>}
      {more && (
        <div hidden={!open} className="flex flex-col gap-1">
          <div
            ref={setDetails}
            className="selectable bg-background rounded-lg px-3 py-2 font-mono text-xs whitespace-pre-wrap wrap-anywhere"
          >
            {more}
          </div>
          <button
            type="button"
            onClick={copy}
            className="hover:text-foreground flex items-center gap-1 self-end text-xs font-semibold"
          >
            {copied ? <Check aria-hidden size={12} /> : <Copy aria-hidden size={12} />}
            {copied ? t('issue.copied') : t('issue.copy')}
          </button>
        </div>
      )}
    </div>
  );
};
