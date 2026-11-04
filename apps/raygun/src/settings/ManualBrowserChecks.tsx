import { TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { PrefSwitch } from './prefs';
import { useSettings, useUpdateSetting } from './useSettings';

const WARNINGS = ['ban', 'terms', 'experimental', 'responsible'] as const;

/**
 * Solve browser checks myself (experimental; `raygun_ext_host::browser_check`): off by default,
 * and turned on only after the warnings are confirmed. Turning it off needs no question.
 */
export const ManualBrowserChecks: FC = () => {
  const { t } = useTranslation();
  const on = useSettings().data?.manualBrowserChecks ?? false;
  const update = useUpdateSetting();
  const [asking, setAsking] = useState(false);

  return (
    <>
      <PrefSwitch
        title={t('sources.browserChecks.title')}
        summary={t('sources.browserChecks.hint')}
        checked={on}
        onChange={(value) =>
          value ? setAsking(true) : update.mutate({ key: 'manualBrowserChecks', value: false })
        }
      />
      {asking && (
        <Sheet title={t('sources.browserChecks.confirmTitle')} onClose={() => setAsking(false)}>
          <div data-testid="browser-checks-warning" className="flex flex-col gap-4 pb-1">
            <ul className="flex flex-col gap-2 text-sm">
              {WARNINGS.map((warning) => (
                <li key={warning} className="flex gap-2">
                  <TriangleAlert aria-hidden size={16} className="text-primary mt-0.5 shrink-0" />
                  <span>{t(`sources.browserChecks.warnings.${warning}`)}</span>
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
                  update.mutate({ key: 'manualBrowserChecks', value: true });
                  setAsking(false);
                }}
                className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
              >
                {t('sources.browserChecks.confirm')}
              </button>
            </div>
          </div>
        </Sheet>
      )}
    </>
  );
};
