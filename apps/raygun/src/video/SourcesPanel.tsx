import { ChevronLeft, Settings2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PanelHeading, PanelOption, PlayerPanel } from './PlayerPanel';
import { sendRequest } from './protocol';
import type { SourceGroup } from './protocol';
import { formatSpeed, leftOutText, optionDetail, optionTitle, sortedOptions } from './qualityText';
import { transferText } from './transferText';
import { useVideoPlayerStore } from './videoPlayerStore';
import { sourceName } from './waiting';

type Props = { positionMs: number; onClose: () => void };

/** Side by side in a landscape window; one after the other in a narrow or upright one. */
const useLandscape = () => {
  const read = () =>
    typeof window === 'undefined' ||
    (window.innerWidth >= 640 && window.innerWidth >= window.innerHeight);
  const [landscape, setLandscape] = useState(read);
  useEffect(() => {
    const update = () => setLandscape(read());
    window.addEventListener('resize', update);
    return () => window.removeEventListener('resize', update);
  }, []);
  return landscape;
};

/**
 * A session's sources (M44; clean list after v1.6.0): two tabs. Sources lists them by clean
 * names (the addon or server, never the streams' long names) in the order Rust gives (what
 * works, the best picture, measured speed), and beside it (or after picking one, in a narrow
 * window) that source's options: resolution, HDR, codec, sound, size and network data, best
 * first; a click plays it from where the video is. Raw details is the full list as the sources
 * name it.
 */
export const SourcesPanel: FC<Props> = ({ positionMs, onClose }) => {
  const { t } = useTranslation();
  const id = useVideoPlayerStore((state) => state.request?.id ?? '');
  const sources = useVideoPlayerStore((state) => state.sources);
  const order = useVideoPlayerStore((state) => state.sourceOrder);
  const playingFrom = useVideoPlayerStore((state) => state.playingFrom);
  const alone = useVideoPlayerStore((state) => state.sourcesAlone);
  const prefs = useVideoPlayerStore((state) => state.prefs);
  // A torrent playing: its peers and speed beside the option playing.
  const transfer = transferText(
    useVideoPlayerStore((state) => state.transfer),
    t,
  );
  const landscape = useLandscape();
  const [raw, setRaw] = useState(false);
  const [picked, setPicked] = useState<number | null>(null);
  const [options, setOptions] = useState(false);
  /** The source whose button was pressed: its settings show. */
  const [settingsOf, setSettingsOf] = useState<number | null>(null);
  const leaveOutTrailers = useVideoPlayerStore((state) => state.leaveOutTrailers);
  const leaveOutShort = useVideoPlayerStore((state) => state.leaveOutShort);
  /** A Sources toggle: shown at once, saved as the setting it is. */
  const toggle = (key: 'leaveOutTrailers' | 'leaveOutShort', on: boolean) => {
    useVideoPlayerStore.setState({ [key]: on });
    sendRequest(id, 'prefs', positionMs, { change: { [key]: on } });
  };
  /** A source's button: off for this title, everywhere, on again, or its settings in the app. */
  const sourceAction = (group: number, action: 'title' | 'everywhere' | 'on' | 'settings') => {
    if (action === 'settings') {
      sendRequest(id, 'sourceSettings', positionMs, { group });
      onClose();
      return;
    }
    sendRequest(id, 'sourceOff', positionMs, { group, scope: action });
    setSettingsOf(null);
  };

  // Sources waiting for a key or sign-in weren't asked: one line counts them. A link's own group
  // always shows (why it failed included).
  const visible = (group: SourceGroup) =>
    group.links.length > 0 ||
    group.own === true ||
    (group.state !== 'setup' &&
      (group.state !== 'failed' || prefs.showFailedSources) &&
      (group.state !== 'empty' || prefs.showEmptySources));
  const waiting = sources.filter((group) => group.state === 'setup' && group.links.length === 0);
  const setupLine = waiting.length > 0 && (
    <p data-testid="sources-need-setup" className="text-muted px-3 py-2 text-sm">
      {t('sourcesSetup.count', { count: waiting.length })}:{' '}
      {waiting.map((group) => group.name).join(', ')}
    </p>
  );
  const all = sources.map((_, index) => index);
  const listed = (order ?? []).filter(
    (index, place, list) => index >= 0 && index < sources.length && list.indexOf(index) === place,
  );
  const shown = [...listed, ...all.filter((index) => !listed.includes(index))].filter((index) => {
    const group = sources[index];
    return group ? visible(group) : false;
  });
  const playing = sources.findIndex((group) => group.links.some((link) => link.current));
  const chosen =
    picked !== null && shown.includes(picked)
      ? picked
      : playing >= 0 && shown.includes(playing)
        ? playing
        : (shown[0] ?? null);

  const choose = (group: number, link: number) => {
    const source = sources[group];
    if (!source || source.links[link]?.current) return;
    // Loads from where the video is; the ring names the source while it does.
    useVideoPlayerStore.setState({
      pickedSource: true,
      notice: null,
      loadingText: t('videoPlayer.loadingSource', {
        n: sourceName(source, link) ?? source.name,
      }),
    });
    sendRequest(id, 'source', positionMs, { group, link });
    onClose();
  };

  const stateText = (group: SourceGroup) =>
    group.state === 'resolving'
      ? t('videoPlayer.resolving')
      : group.state === 'failed'
        ? `${t('videoPlayer.failed')}${group.error ? `: ${group.error.slice(0, 120)}` : ''}`
        : t('videoPlayer.empty');
  const standing = (group: SourceGroup) =>
    group.unreliable
      ? t('videoPlayer.unreliable')
      : group.worked
        ? t('videoPlayer.workedLastTime')
        : null;
  const sourceDetail = (group: SourceGroup, index: number) =>
    group.links.length === 0
      ? stateText(group)
      : [
          index === playing ? t('videoPlayer.sourcePlaying') : null,
          group.best ? t('videoPlayer.sourceBest', { n: `${group.best}p` }) : null,
          t('videoPlayer.sourceOptions', {
            n: group.options ?? group.links.length,
            count: group.options ?? group.links.length,
          }),
          group.speedKbps ? formatSpeed(group.speedKbps) : null,
          standing(group),
        ]
          .filter(Boolean)
          .join(' · ');

  const sourceList = (
    <div className="flex flex-col">
      {shown.map((index) => {
        const group = sources[index];
        if (!group) return null;
        const hasButton = group.kind !== undefined && group.kind !== 'none';
        return (
          <div key={`${index}-${group.name}`} className="flex items-center gap-1">
            <div className="min-w-0 flex-1">
              <PanelOption
                label={group.name}
                hint={
                  group.off
                    ? [t('videoPlayer.sourceOffHere'), sourceDetail(group, index)].join(' · ')
                    : sourceDetail(group, index)
                }
                selected={landscape && index === chosen}
                current={index === playing}
                onSelect={() => {
                  setPicked(index);
                  setOptions(true);
                }}
              />
            </div>
            {hasButton && (
              <button
                type="button"
                aria-label={t('videoPlayer.sourceSettings', { n: group.name })}
                aria-expanded={settingsOf === index}
                onClick={() => setSettingsOf(settingsOf === index ? null : index)}
                className="grid size-9 shrink-0 place-items-center rounded-lg text-white/70 hover:bg-white/10"
              >
                <Settings2 aria-hidden size={16} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );

  const optionList = (index: number | null) => {
    const group = index === null ? undefined : sources[index];
    if (index === null || !group) return null;
    if (group.links.length === 0) {
      return <p className="px-3 py-2 text-sm text-white/60">{stateText(group)}</p>;
    }
    return (
      <div className="flex flex-col" aria-label={group.name} role="group">
        {sortedOptions(group).map(({ link, index: linkIndex }) => (
          <PanelOption
            key={`${linkIndex}-${link.label}`}
            label={optionTitle(link, t)}
            hint={
              [leftOutText(link, t), optionDetail(link, group, t), link.current ? transfer : null]
                .filter(Boolean)
                .join(' · ') || null
            }
            selected={link.current}
            current={link.current}
            onSelect={() => choose(index, linkIndex)}
          />
        ))}
      </div>
    );
  };

  const rawList = sources.map((group, index) =>
    visible(group) ? (
      <div key={`${index}-${group.name}`} className="flex flex-col">
        <PanelHeading>
          {group.name}
          {group.links.length === 0 && ` · ${stateText(group)}`}
          {standing(group) && ` · ${standing(group)}`}
        </PanelHeading>
        {group.links.map((link, linkIndex) => (
          <PanelOption
            key={`${linkIndex}-${link.label}`}
            label={link.label}
            hint={
              link.failed
                ? t('videoPlayer.failed')
                : link.current
                  ? t('videoPlayer.sourcePlaying')
                  : link.worked
                  ? t('videoPlayer.workedLastTime')
                  : leftOutText(link, t)
            }
            selected={link.current}
            current={link.current}
            onSelect={() => choose(index, linkIndex)}
          />
        ))}
      </div>
    ) : null,
  );

  const tab = (isRaw: boolean, label: string) => (
    <button
      type="button"
      role="tab"
      aria-selected={raw === isRaw}
      onClick={() => setRaw(isRaw)}
      className={`rounded-full px-3 py-1.5 text-sm font-semibold ${
        raw === isRaw ? 'bg-white text-black' : 'bg-white/10 hover:bg-white/20'
      }`}
    >
      {label}
    </button>
  );

  return (
    <PlayerPanel title={t('videoPlayer.sources')} onClose={onClose} at="top" wide>
      <div role="tablist" className="flex gap-2 px-2 pb-2">
        {tab(false, t('videoPlayer.sourcesTab'))}
        {tab(true, t('videoPlayer.sourcesRaw'))}
      </div>
      {playingFrom && (
        <p className="text-muted px-3 pb-1 text-sm">
          {t('videoPlayer.playingFrom', { n: playingFrom })}
        </p>
      )}
      {/* A link played on its own with nothing else to switch to: Sources stays, and says so. */}
      {alone && (
        <p data-testid="sources-alone" className="px-3 pb-1 text-sm text-white/70">
          {t('videoPlayer.onlyThisLink')}
        </p>
      )}
      <PanelOption
        label={t('videoPlayer.leaveOutTrailers')}
        selected={leaveOutTrailers}
        toggle
        onSelect={() => toggle('leaveOutTrailers', !leaveOutTrailers)}
      />
      <PanelOption
        label={t('videoPlayer.leaveOutShort')}
        hint={t('videoPlayer.leaveOutShortHint')}
        selected={leaveOutShort}
        toggle
        onSelect={() => toggle('leaveOutShort', !leaveOutShort)}
      />
      {settingsOf !== null && sources[settingsOf] && (
        <div role="group" aria-label={sources[settingsOf].name} className="flex flex-col">
          <PanelHeading>{sources[settingsOf].name}</PanelHeading>
          {sources[settingsOf].off ? (
            <PanelOption
              label={t('videoPlayer.sourceOnHere')}
              onSelect={() => sourceAction(settingsOf, 'on')}
            />
          ) : (
            <PanelOption
              label={t('videoPlayer.sourceOffTitle')}
              onSelect={() => sourceAction(settingsOf, 'title')}
            />
          )}
          <PanelOption
            label={t('videoPlayer.sourceOffEverywhere')}
            onSelect={() => sourceAction(settingsOf, 'everywhere')}
          />
          <PanelOption
            label={t('videoPlayer.sourceOpenSettings')}
            onSelect={() => sourceAction(settingsOf, 'settings')}
          />
        </div>
      )}
      {raw ? (
        rawList
      ) : landscape ? (
        <div className="grid grid-cols-[2fr_3fr] gap-1">
          <div className="max-h-[26rem] overflow-y-auto border-r border-white/10 pr-1">
            {sourceList}
          </div>
          <div className="max-h-[26rem] overflow-y-auto">{optionList(chosen)}</div>
        </div>
      ) : options && chosen !== null ? (
        <>
          <button
            type="button"
            onClick={() => setOptions(false)}
            className="flex items-center gap-1 rounded-lg px-3 py-2 text-left text-sm hover:bg-white/10"
          >
            <ChevronLeft aria-hidden size={16} />
            {t('videoPlayer.allSources')}
          </button>
          <PanelHeading>{sources[chosen]?.name}</PanelHeading>
          {optionList(chosen)}
        </>
      ) : (
        sourceList
      )}
      {setupLine}
    </PlayerPanel>
  );
};
