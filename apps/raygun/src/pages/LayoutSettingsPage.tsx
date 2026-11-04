import type { LayoutItem } from '@raygun/bindings';
import { useParams } from '@tanstack/react-router';
import { RotateCcw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { isLayoutSurface, LAYOUT_DEFAULTS, LAYOUT_SURFACES, LOCKED } from '../layout/layout';
import type { LayoutSurface } from '../layout/layout';
import { LayoutEditor } from '../layout/LayoutEditor';
import { surfaceEntries } from '../layout/surfaces';
import { useLayout, useSaveLayout } from '../layout/useLayout';
import { PrefGroup, PrefLink } from '../settings/prefs';
import { TopBar } from '../shell/TopBar';

/** Settings → Appearance → Layout (M46.5): every list the user can arrange. */
export const LayoutSettingsPage: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  return (
    <section data-testid="layout-settings" className="flex max-w-2xl flex-col gap-4">
      <TopBar back fallback="/settings/appearance" title={t('layout.title')} />
      <PrefGroup>
        {LAYOUT_SURFACES.map((surface) => {
          const entries = surfaceEntries(
            surface,
            layout[surface].map((item) => item.id),
            t,
          );
          const shown = layout[surface]
            .filter((item) => item.visible)
            .map((item) => entries[item.id]?.label ?? item.id);
          return (
            <PrefLink
              key={surface}
              title={t(`layout.surfaces.${surface}`)}
              summary={shown.join(' · ')}
              to={`/settings/layout/${surface}`}
            />
          );
        })}
      </PrefGroup>
    </section>
  );
};

/** One list: drag or move entries, show or hide them, or go back to the default. */
export const LayoutSurfacePage: FC = () => {
  const { t } = useTranslation();
  const { surface } = useParams({ strict: false });
  const valid = isLayoutSurface(surface);
  return (
    <section data-testid="layout-surface" className="flex max-w-2xl flex-col gap-4">
      <TopBar
        back
        fallback="/settings/layout"
        title={valid ? t(`layout.surfaces.${surface}`) : t('layout.title')}
      />
      {valid ? (
        <SurfaceEditor key={surface} surface={surface} />
      ) : (
        <p className="text-muted px-1 text-sm">{t('layout.unknown')}</p>
      )}
    </section>
  );
};

const SurfaceEditor: FC<{ surface: LayoutSurface }> = ({ surface }) => {
  const { t } = useTranslation();
  const layout = useLayout();
  const saving = useSaveLayout();
  // Shown at once while it saves, so a dropped row doesn't jump back.
  const [draft, setDraft] = useState<LayoutItem[] | null>(null);
  const items = draft ?? layout[surface];
  const change = (next: LayoutItem[]) => {
    setDraft(next);
    saving.save(surface, next, () => setDraft(null));
  };
  const isDefault = JSON.stringify(layout[surface]) === JSON.stringify(LAYOUT_DEFAULTS[surface]);

  return (
    <>
      <p className="text-muted px-1 text-sm">{t(`layout.surfaceHints.${surface}`)}</p>
      <LayoutEditor
        items={items}
        entries={surfaceEntries(
          surface,
          items.map((item) => item.id),
          t,
        )}
        locked={LOCKED[surface] ?? []}
        onChange={change}
      />
      {saving.error && (
        <p role="alert" className="text-primary px-1 text-sm">
          {saving.error.message}
        </p>
      )}
      <button
        type="button"
        disabled={isDefault || saving.isPending}
        onClick={() => change(LAYOUT_DEFAULTS[surface])}
        className="bg-surface flex w-fit items-center gap-2 rounded-full px-4 py-2 text-sm font-medium disabled:opacity-50"
      >
        <RotateCcw aria-hidden size={16} />
        {t('layout.reset')}
      </button>
    </>
  );
};
