import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlayerUiStore } from '../player/playerUiStore';
import { useIncognito } from '../privacy/useIncognito';
import { runPlaybackAction } from '../player/usePlaybackKeys';
import type { PlaybackAction } from '../player/usePlaybackKeys';
import type { AppMenuGroup, AppMenuItem } from './AppMenu';
import { closeWindow } from './appWindow';
import { toggleWindowFullscreen } from './useWindowFullscreen';

type Field = HTMLInputElement | HTMLTextAreaElement;

const isField = (target: EventTarget | null): target is Field =>
  target instanceof HTMLTextAreaElement ||
  (target instanceof HTMLInputElement &&
    !['checkbox', 'radio', 'range', 'color', 'button', 'submit', 'file'].includes(target.type));

/** The text field the Edit menu works on: the last one focused before the menu took focus. */
const useLastField = () => {
  const field = useRef<Field | null>(null);
  useEffect(() => {
    const onFocus = (event: FocusEvent) => {
      if (isField(event.target)) field.current = event.target;
    };
    document.addEventListener('focusin', onFocus);
    return () => document.removeEventListener('focusin', onFocus);
  }, []);
  return field;
};

const changed = (field: Field) => field.dispatchEvent(new Event('input', { bubbles: true }));
const selectedIn = (field: Field) =>
  field.value.slice(field.selectionStart ?? 0, field.selectionEnd ?? 0);

/** What the app menu needs from the app bar. */
export type AppMenuActions = {
  newPlaylist: () => void;
  importComics: () => void;
  importBook: () => void;
  focusSearch: () => void;
  zoom: (by: number | null) => void;
  shortcuts: () => void;
};

/**
 * The app bar menu's groups (M46.9): File, Edit, View, Playback and Help, as Spotify's on Windows.
 * Call `snapshot` when the menu opens: Edit's items depend on the field and text selected then.
 */
export const useAppMenuGroups = (actions: AppMenuActions) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const lastField = useLastField();
  const incognito = useIncognito();
  const [{ field, selected }, setSeen] = useState({ field: null as Field | null, selected: '' });

  const snapshot = () => {
    const current = lastField.current?.isConnected ? lastField.current : null;
    setSeen({
      field: current,
      selected: current ? selectedIn(current) : (window.getSelection()?.toString() ?? ''),
    });
  };

  const inField = (run: (field: Field) => void) => () => {
    if (!field) return;
    field.focus();
    run(field);
  };
  const copy = (text: string) => void navigator.clipboard?.writeText(text).catch(() => undefined);
  const play = (action: PlaybackAction, hint: string): AppMenuItem => ({
    label: t(`shortcuts.${action}`),
    hint,
    onSelect: () => void runPlaybackAction(action, client),
  });
  const ui = usePlayerUiStore.getState;

  const groups: AppMenuGroup[] = [
    {
      id: 'file',
      label: t('appBar.file'),
      items: [
        { label: t('collections.newPlaylist'), hint: 'Ctrl+N', onSelect: actions.newPlaylist },
        { label: t('manga.importComics'), onSelect: actions.importComics },
        { label: t('novel.importEpub'), onSelect: actions.importBook },
        { kind: 'separator' },
        {
          label: t(incognito.on ? 'incognito.turnOff' : 'incognito.turnOn'),
          onSelect: () => incognito.set(!incognito.on),
        },
        { kind: 'separator' },
        {
          label: t('settings.title'),
          hint: 'Ctrl+,',
          onSelect: () => void navigate({ to: '/settings' }),
        },
        { kind: 'separator' },
        { label: t('appBar.exit'), hint: 'Alt+F4', onSelect: () => void closeWindow() },
      ],
    },
    {
      id: 'edit',
      label: t('appBar.edit'),
      items: [
        {
          label: t('appBar.undo'),
          hint: 'Ctrl+Z',
          disabled: !field,
          onSelect: inField(() => document.execCommand('undo')),
        },
        {
          label: t('appBar.redo'),
          hint: 'Ctrl+Y',
          disabled: !field,
          onSelect: inField(() => document.execCommand('redo')),
        },
        { kind: 'separator' },
        {
          label: t('menu.cut'),
          hint: 'Ctrl+X',
          disabled: !field || !selected,
          onSelect: inField((target) => {
            copy(selected);
            target.setRangeText('', target.selectionStart ?? 0, target.selectionEnd ?? 0, 'end');
            changed(target);
          }),
        },
        {
          label: t('menu.copy'),
          hint: 'Ctrl+C',
          disabled: !selected,
          onSelect: () => copy(selected),
        },
        {
          label: t('menu.paste'),
          hint: 'Ctrl+V',
          disabled: !field,
          onSelect: inField((target) => {
            void navigator.clipboard
              ?.readText()
              .then((text) => {
                target.setRangeText(
                  text,
                  target.selectionStart ?? 0,
                  target.selectionEnd ?? 0,
                  'end',
                );
                changed(target);
              })
              .catch(() => undefined);
          }),
        },
        {
          label: t('appBar.delete'),
          hint: 'Del',
          disabled: !field || !selected,
          onSelect: inField((target) => {
            target.setRangeText('', target.selectionStart ?? 0, target.selectionEnd ?? 0, 'end');
            changed(target);
          }),
        },
        { kind: 'separator' },
        {
          label: t('menu.selectAll'),
          hint: 'Ctrl+A',
          disabled: !field,
          onSelect: inField((target) => target.select()),
        },
      ],
    },
    {
      id: 'view',
      label: t('appBar.view'),
      items: [
        { label: t('nav.home'), onSelect: () => void navigate({ to: '/' }) },
        { label: t('nav.library'), onSelect: () => void navigate({ to: '/library' }) },
        { label: t('nav.browse'), onSelect: () => void navigate({ to: '/browse' }) },
        { label: t('appBar.search'), hint: 'Ctrl+K', onSelect: actions.focusSearch },
        { kind: 'separator' },
        { label: t('player.nowPlaying'), onSelect: () => ui().togglePanel('nowPlaying') },
        { label: t('shortcuts.queue'), hint: 'Q', onSelect: () => ui().togglePanel('queue') },
        {
          label: t('shortcuts.lyrics'),
          hint: 'L',
          onSelect: () => ui().openSheet({ view: 'lyrics' }),
        },
        {
          label: t('appBar.fullPlayer'),
          hint: 'P',
          onSelect: () => void runPlaybackAction('player', client),
        },
        { kind: 'separator' },
        { label: t('appBar.zoomIn'), hint: 'Ctrl+=', onSelect: () => actions.zoom(10) },
        { label: t('appBar.zoomOut'), hint: 'Ctrl+-', onSelect: () => actions.zoom(-10) },
        { label: t('appBar.zoomReset'), hint: 'Ctrl+0', onSelect: () => actions.zoom(null) },
        {
          label: t('shortcuts.fullscreen'),
          hint: 'F11',
          onSelect: () => void toggleWindowFullscreen(),
        },
      ],
    },
    {
      id: 'playback',
      label: t('appBar.playback'),
      items: [
        play('playPause', 'Space'),
        play('next', 'Ctrl+→'),
        play('previous', 'Ctrl+←'),
        play('forward', 'Shift+→'),
        play('back', 'Shift+←'),
        { kind: 'separator' },
        play('shuffle', 'S'),
        play('repeat', 'R'),
        { kind: 'separator' },
        play('louder', 'Ctrl+↑'),
        play('quieter', 'Ctrl+↓'),
        play('mute', 'M'),
      ],
    },
    {
      id: 'help',
      label: t('appBar.help'),
      items: [
        { label: t('shortcuts.title'), hint: '?', onSelect: actions.shortcuts },
        { label: t('appBar.about'), onSelect: () => void navigate({ to: '/about' }) },
      ],
    },
  ];
  return { groups, snapshot };
};
