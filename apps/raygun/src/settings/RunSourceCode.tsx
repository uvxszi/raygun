import { TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { PrefSwitch } from './prefs';
import { useSettings, useUpdateSetting } from './useSettings';

const NOTES = ['code', 'checked', 'unavailable'] as const;

/**
 * Allow running source code (`Settings::run_source_code`): Mihon extensions Raygun can't convert
 * may be downloaded as their APKs, checked against their store. Off by default, turned on only
 * after the notes are read; turning it off needs no question.
 */
export const RunSourceCode: FC = () => {
  const { t } = useTranslation();
  const on = useSettings().data?.runSourceCode ?? false;
  const update = useUpdateSetting();
  const [asking, setAsking] = useState(false);

  return (
    <>
      <PrefSwitch
        title={t('sources.sourceCode.title')}
        summary={t('sources.sourceCode.hint')}
        checked={on}
        onChange={(value) =>
          value ? setAsking(true) : update.mutate({ key: 'runSourceCode', value: false })
        }
      />
      {asking && (
        <Sheet title={t('sources.sourceCode.confirmTitle')} onClose={() => setAsking(false)}>
          <div data-testid="source-code-notes" className="flex flex-col gap-4 pb-1">
            <ul className="flex flex-col gap-2 text-sm">
              {NOTES.map((note) => (
                <li key={note} className="flex gap-2">
                  <TriangleAlert aria-hidden size={16} className="text-primary mt-0.5 shrink-0" />
                  <span>{t(`sources.sourceCode.notes.${note}`)}</span>
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setAsking(false)}
                className="hover:bg-background rounded-full px-4 py-2 text-sm font-semibold"
              >
                {t('settings.cancel')}
              </button>
              <button
                type="button"
                onClick={() => {
                  update.mutate({ key: 'runSourceCode', value: true });
                  setAsking(false);
                }}
                className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
              >
                {t('sources.sourceCode.confirm')}
              </button>
            </div>
          </div>
        </Sheet>
      )}
    </>
  );
};
