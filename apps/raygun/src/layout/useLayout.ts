import type { LayoutItem } from '@raygun/bindings';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { withLayout } from './layout';
import type { Layout, LayoutSurface } from './layout';

/** The app's arrangement (M46.5), whole even before settings load. */
export const useLayout = (): Layout => withLayout(useSettings().data?.layout);

/** Saves one surface's list; Rust refuses hiding what must stay (More, Settings, Search). */
export const useSaveLayout = () => {
  const layout = useLayout();
  const update = useUpdateSetting();
  return {
    save: (surface: LayoutSurface, items: LayoutItem[], onSettled?: () => void) =>
      update.mutate({ key: 'layout', value: { ...layout, [surface]: items } }, { onSettled }),
    error: update.error,
    isPending: update.isPending,
  };
};
