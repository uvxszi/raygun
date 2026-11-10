import { useEffect } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useBackLayer } from '../shell/backLayers';
import { PickTarget } from './PickTarget';
import { ScopeContext, useInSelectionScope } from './scopeContext';
import { pickedKey, useSelectionStore } from './selectionStore';
import type { Picked } from './selectionStore';
import { SelectionBar } from './SelectionBar';

/**
 * Results inside it can be picked (search, a source's page). It hosts the selection's bar, and
 * the selection goes when it closes, on Back and on Escape.
 */
export const SelectionScope: FC<{ children: ReactNode }> = ({ children }) => {
  const selecting = useSelectionStore((state) => state.items.length > 0);
  const clear = useSelectionStore((state) => state.clear);
  useBackLayer(selecting, clear);
  useEffect(() => clear, [clear]);
  return (
    <ScopeContext.Provider value>
      {children}
      {selecting && <SelectionBar />}
    </ScopeContext.Provider>
  );
};

type Props = {
  picked: Picked;
  children: ReactNode;
  className?: string;
  /** The circle sits on a cover's top left corner (grids) or over the artwork at the row's start. */
  layout?: 'cover' | 'row';
  /** Shown as it is, not pickable (a library cover among results). */
  disabled?: boolean;
};

/**
 * A result that can be picked: a long press (touch), a right click, Ctrl/Cmd/Shift-click or its
 * circle picks it, and while anything is picked a tap adds or takes out instead of opening. Its ⋮
 * and play button keep working. Outside a scope it's just its children.
 */
export const Selectable: FC<Props> = (props) =>
  useInSelectionScope() && !props.disabled ? <SelectableItem {...props} /> : <>{props.children}</>;

const SelectableItem: FC<Props> = ({ picked, children, className = '', layout = 'cover' }) => {
  const { t } = useTranslation();
  const key = pickedKey(picked);
  const selected = useSelectionStore((state) => state.keys.has(key));
  const selecting = useSelectionStore((state) => state.items.length > 0);
  const toggle = useSelectionStore((state) => state.toggle);
  const flip = () => toggle(picked);
  return (
    <PickTarget
      pick={{ selected, selecting, toggle: flip, extend: flip }}
      label={t('search.select.pick', { title: pickedTitle(picked) })}
      place={layout}
      className={className}
      testId="selectable"
    >
      {children}
    </PickTarget>
  );
};

const pickedTitle = (picked: Picked) =>
  picked.kind === 'song' ? picked.track.title : picked.title;
