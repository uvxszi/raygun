import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { NeedsSetupNote } from '../shell/NeedsSetupNote';
import type { SetupSource } from '../shell/NeedsSetupNote';
import { SourceIssue } from '../shell/SourceIssue';
import { setupSourceOf } from '../sources/useSourcesSetup';

/**
 * Sources that couldn't search, quietly after the results: one line naming the source (or how
 * many), each one's error under More info. Sources that need a key or sign-in aren't failures:
 * they're counted in one line of their own, with every other source of `kinds` waiting for one.
 */
export const FailedSources: FC<{
  failed: { name: string; error: string }[];
  /** The provider kinds the search asked, so sources it left out for want of a key count too. */
  kinds?: readonly string[];
  /** False where the page shows the sources that need setup in a line of its own. */
  setupNote?: boolean;
}> = ({ failed, kinds, setupNote = true }) => {
  const { t } = useTranslation();
  const setup: SetupSource[] = [];
  const others: { name: string; error: string }[] = [];
  for (const source of failed) {
    const waiting = setupSourceOf(source.error, source.name);
    if (waiting) setup.push(waiting);
    else others.push(source);
  }
  const note = setupNote && <NeedsSetupNote kinds={kinds} sources={setup} />;
  if (others.length === 0) return note || null;
  // One source: its error read into what happened (M46.48), the raw details under More info.
  if (others.length === 1) {
    return (
      <>
        <SourceIssue compact testId="failed-sources" source={others[0]!.name}>
          {others[0]!.error}
        </SourceIssue>
        {note}
      </>
    );
  }
  return (
    <>
      <SourceIssue
        compact
        testId="failed-sources"
        source={t('search.sourcesCount', { count: others.length })}
      >
        {others.map((source) => (
          <span key={source.name} className="block">
            {source.name}: {source.error}
          </span>
        ))}
      </SourceIssue>
      {note}
    </>
  );
};
