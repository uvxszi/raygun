import { ChevronLeft } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PanelHeading, PanelOption, PlayerPanel } from './PlayerPanel';
import { sendRequest } from './protocol';
import type { SourceGroup } from './protocol';
import { formatSpeed, optionDetail, optionTitle, sortedOptions } from './qualityText';
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
  const prefs = useVideoPlayerStore((state) => state.prefs);
  const landscape = useLandscape();
  const [raw, setRaw] = useState(false);
  const [picked, setPicked] = useState<number | null>(null);
  const [options, setOptions] = useState(false);

  const visible = (group: SourceGroup) =>
    group.links.length > 0 ||
    ((group.state !== 'failed' || prefs.showFailedSources) &&
      (group.state !== 'empty' || prefs.showEmptySources));
  const all = sources.map((_, index) => index);
  const listed = (order ?? []).filter(
    (index, place, list) =>
      index >= 0 && index < sources.length && list.indexOf(index) === place,
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
        return (
          <PanelOption
            key={`${index}-${group.name}`}
            label={group.name}
            hint={sourceDetail(group, index)}
            selected={landscape && index === chosen}
            onSelect={() => {
              setPicked(index);
              setOptions(true);
            }}
          />
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
            hint={optionDetail(link, group, t)}
            selected={link.current}
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
                : link.worked
                  ? t('videoPlayer.workedLastTime')
                  : null
            }
            selected={link.current}
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
    </PlayerPanel>
  );
};
