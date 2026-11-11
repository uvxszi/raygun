import { commands } from '@raygun/bindings';
import { useRouter } from '@tanstack/react-router';
import type { ErrorComponentProps } from '@tanstack/react-router';
import { AlertTriangle } from 'lucide-react';
import { Component, useEffect, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { hideSplash } from './splash';

/** An error's one line and its whole account, for the crash log. */
const describeError = (error: unknown): { summary: string; detail: string } => {
  if (error instanceof Error) {
    const summary = `${error.name}: ${error.message}`.split('\n')[0] ?? error.name;
    return { summary, detail: `${error.name}: ${error.message}\n\n${error.stack ?? ''}` };
  }
  const text = typeof error === 'string' ? error : (JSON.stringify(error) ?? String(error));
  return { summary: text.split('\n')[0] ?? text, detail: text };
};

type ErrorScreenProps = {
  error: unknown;
  /** Shows the page again (the route's own retry, or a reload). */
  onRetry: () => void;
  /** Opens the crash logs at the record this error left (`null` when none could be kept). */
  onSeeLogs: (id: string | null) => void;
};

/**
 * What shows when Raygun's page can't draw (a route's error, or one no route caught): a plain
 * message in Raygun's look, never the raw error. The error goes to a crash log (About → Crash
 * logs) as the screen shows; See logs opens it. Start in safe mode and Clear caches start Raygun
 * again as a safe start (`crash_log.rs`).
 */
export const ErrorScreen: FC<ErrorScreenProps> = ({ error, onRetry, onSeeLogs }) => {
  const { t } = useTranslation();
  const [recordId, setRecordId] = useState<string | null>(null);

  useEffect(() => {
    // The start screen would otherwise stay over this until its own limit.
    hideSplash({ fade: false });
    const { summary, detail } = describeError(error);
    let current = true;
    commands
      .crashLogWrite(summary, detail)
      .then((result) => {
        if (current && result.status === 'ok') setRecordId(result.data);
      })
      .catch(() => {
        // Not kept: the screen still offers everything else.
      });
    return () => {
      current = false;
    };
  }, [error]);

  const restart = (clearCaches: boolean) => {
    void commands.appRestartSafe(clearCaches).catch(() => window.location.reload());
  };

  const button =
    'w-full rounded-full px-4 py-3 text-base font-semibold transition-colors focus-visible:outline-2';
  return (
    <main
      role="alert"
      data-testid="error-screen"
      className="bg-background text-foreground flex min-h-dvh items-center justify-center"
      style={{
        paddingTop: 'max(1.5rem, var(--safe-top, 0px))',
        paddingBottom: 'max(1.5rem, var(--safe-bottom, 0px))',
        paddingLeft: 'max(1.5rem, var(--safe-left, 0px))',
        paddingRight: 'max(1.5rem, var(--safe-right, 0px))',
      }}
    >
      <div className="flex w-full max-w-sm flex-col items-center gap-3 text-center">
        <AlertTriangle aria-hidden size={40} className="text-primary" />
        <h1 className="text-2xl font-bold">{t('crash.title')}</h1>
        <p>{t('crash.body')}</p>
        <p className="text-muted text-sm">{t('crash.hint')}</p>
        <div className="mt-3 flex w-full flex-col gap-2.5">
          <button type="button" className={`${button} bg-primary text-white`} onClick={onRetry}>
            {t('crash.tryAgain')}
          </button>
          <button
            type="button"
            className={`${button} bg-surface hover:bg-surface/70`}
            onClick={() => onSeeLogs(recordId)}
          >
            {t('crash.seeLogs')}
          </button>
          <button
            type="button"
            className={`${button} bg-surface hover:bg-surface/70`}
            onClick={() => restart(false)}
          >
            {t('crash.safeMode')}
          </button>
          <button
            type="button"
            className={`${button} bg-surface hover:bg-surface/70`}
            onClick={() => restart(true)}
          >
            {t('crash.clearCaches')}
          </button>
        </div>
      </div>
    </main>
  );
};

/** The crash logs page, reached without the router (it may be what failed). */
const openLogsByAddress = (id: string | null) => {
  window.location.hash = id ? `#/about/crashes?id=${encodeURIComponent(id)}` : '#/about/crashes';
  window.location.reload();
};

/** A route that failed to draw or load: its own retry, and the crash logs through the router. */
export const RouteError: FC<ErrorComponentProps> = ({ error, reset }) => {
  const router = useRouter();
  return (
    <ErrorScreen
      error={error}
      onRetry={() => {
        reset();
        void router.invalidate();
      }}
      onSeeLogs={(id) => {
        reset();
        void router.navigate({ to: '/about/crashes', search: { id: id ?? undefined } });
      }}
    />
  );
};

type BoundaryState = { error: unknown };

/**
 * Catches what no route did (the shell, the providers): the error screen with a reload. React
 * catches render errors only in a class component.
 */
export class RootErrorBoundary extends Component<{ children: ReactNode }, BoundaryState> {
  state: BoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): BoundaryState {
    return { error: error ?? new Error('Unknown error') };
  }

  render() {
    if (this.state.error === null) return this.props.children;
    return (
      <ErrorScreen
        error={this.state.error}
        onRetry={() => window.location.reload()}
        onSeeLogs={openLogsByAddress}
      />
    );
  }
}
