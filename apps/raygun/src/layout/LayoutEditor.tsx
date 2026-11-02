import type { LayoutItem } from '@raygun/bindings';
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import type { DragEndEvent } from '@dnd-kit/core';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { ChevronDown, ChevronUp, Eye, EyeOff, GripVertical, Lock } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

export type LayoutEntry = { label: string; icon?: LucideIcon };

type Props = {
  items: LayoutItem[];
  entries: Record<string, LayoutEntry>;
  /** Entries that always show. */
  locked: string[];
  onChange: (items: LayoutItem[]) => void;
};

/**
 * One list the user arranges (M46.5): drag by the handle (pointer, touch or keys), or move with
 * the arrows; the eye shows or hides an entry. Mihon's category editor, with TachiyomiAT's arrows.
 */
export const LayoutEditor: FC<Props> = ({ items, entries, locked, onChange }) => {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const ids = items.map((item) => item.id);
  const move = (from: number, to: number) => {
    if (to < 0 || to >= items.length || from === to) return;
    onChange(arrayMove(items, from, to));
  };
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    move(ids.indexOf(String(active.id)), ids.indexOf(String(over.id)));
  };

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
      <SortableContext items={ids} strategy={verticalListSortingStrategy}>
        <ul data-testid="layout-editor" className="flex flex-col gap-1">
          {items.map((item, index) => (
            <EditorRow
              key={item.id}
              item={item}
              entry={entries[item.id] ?? { label: item.id }}
              locked={locked.includes(item.id)}
              first={index === 0}
              last={index === items.length - 1}
              onMove={(step) => move(index, index + step)}
              onToggle={() =>
                onChange(
                  items.map((other) =>
                    other.id === item.id ? { ...other, visible: !other.visible } : other,
                  ),
                )
              }
            />
          ))}
        </ul>
      </SortableContext>
    </DndContext>
  );
};

type RowProps = {
  item: LayoutItem;
  entry: LayoutEntry;
  locked: boolean;
  first: boolean;
  last: boolean;
  onMove: (step: -1 | 1) => void;
  onToggle: () => void;
};

const EditorRow: FC<RowProps> = ({ item, entry, locked, first, last, onMove, onToggle }) => {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: item.id,
  });
  const name = entry.label;
  const Icon = entry.icon;
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      data-testid={`layout-row-${item.id}`}
      className={`bg-surface flex items-center gap-1 rounded-xl py-1 pr-1 pl-1 ${isDragging ? 'relative z-10 shadow-lg' : ''}`}
    >
      <button
        type="button"
        aria-label={t('layout.drag', { name })}
        {...attributes}
        {...listeners}
        style={{ touchAction: 'none' }}
        className="text-muted grid size-10 shrink-0 cursor-grab place-items-center rounded-full active:cursor-grabbing"
      >
        <GripVertical aria-hidden size={18} />
      </button>
      {Icon && (
        <Icon aria-hidden size={20} className={item.visible ? 'text-primary' : 'text-muted'} />
      )}
      <span
        className={`min-w-0 flex-1 truncate px-2 ${item.visible ? '' : 'text-muted line-through'}`}
      >
        {name}
      </span>
      <button
        type="button"
        aria-label={t('layout.moveUp', { name })}
        disabled={first}
        onClick={() => onMove(-1)}
        className="grid size-10 place-items-center rounded-full disabled:opacity-30"
      >
        <ChevronUp aria-hidden size={18} />
      </button>
      <button
        type="button"
        aria-label={t('layout.moveDown', { name })}
        disabled={last}
        onClick={() => onMove(1)}
        className="grid size-10 place-items-center rounded-full disabled:opacity-30"
      >
        <ChevronDown aria-hidden size={18} />
      </button>
      {locked ? (
        <span
          title={t('layout.locked')}
          aria-label={t('layout.locked')}
          className="text-muted grid size-10 place-items-center"
        >
          <Lock aria-hidden size={16} />
        </span>
      ) : (
        <button
          type="button"
          role="switch"
          aria-checked={item.visible}
          aria-label={t('layout.show', { name })}
          onClick={onToggle}
          className={`grid size-10 place-items-center rounded-full ${item.visible ? 'text-primary' : 'text-muted'}`}
        >
          {item.visible ? <Eye aria-hidden size={18} /> : <EyeOff aria-hidden size={18} />}
        </button>
      )}
    </li>
  );
};
