import { FilePlus2 } from 'lucide-react';
import { useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { DelayControls } from './DelayControls';
import type { SoundSource } from './externalAudio';
import { PanelHeading, PanelOption } from './PlayerPanel';
import type { AudioOffer } from './protocol';
import { languageCode, shownTrackName } from './trackPicking';

/** One of the video's own audio tracks, named apart from the others. */
export type OwnAudioTrack = { id: number; name: string; detail: string | null };

type Props = {
  tracks: OwnAudioTrack[];
  /** The video's own track playing (when its own sound is heard). */
  playingTrack: number | null;
  /** The video's own sound again: a track of it, or -1 for a stream that lists none. */
  onTrack: (id: number) => void;
  /** The sound beside the picture, when one was picked (another source's, a file). */
  sound: SoundSource | null;
  /** Other sources' streams of the episode, for their sound. */
  offers: AudioOffer[];
  /** A session's video: other sources may offer their sound. */
  session: boolean;
  onOffer: (offer: AudioOffer) => void;
  delayMs: number;
  onDelay: (ms: number) => void;
};

/** A language code's name in the app's language ("it" → "Italian"). */
const languageName = (code: string) => {
  const known = languageCode(code);
  return known ? shownTrackName(known, known) : code.toUpperCase();
};

/**
 * Audio, under smart audio's switches (Add an audio file is by the title, `AddAudioFileButton`):
 * the audio delay (later or earlier than the picture, kept for the title), the video's own
 * tracks with the sound beside the picture, then the other sources' streams of this episode,
 * each named by the languages its names give and where it comes from. Picking one plays its
 * sound over this picture.
 */
export const AudioPanel: FC<Props> = ({
  tracks,
  playingTrack,
  onTrack,
  sound,
  offers,
  session,
  onOffer,
  delayMs,
  onDelay,
}) => {
  const { t } = useTranslation();
  const offerName = (offer: AudioOffer) => {
    const languages = [...new Set(offer.languages.map(languageName))];
    if (languages.length > 0) return languages.join(', ');
    return offer.several ? t('videoPlayer.audioSeveral') : t('videoPlayer.audioUnnamed');
  };
  const offerDetail = (offer: AudioOffer) =>
    [offer.source, offer.title, offer.sound].filter(Boolean).join(' · ') || null;
  const beside = sound && !sound.own ? sound : null;

  return (
    <>
      <DelayControls
        delayMs={delayMs}
        label={t('videoPlayer.audioDelaySeconds')}
        onChange={onDelay}
      />
      <PanelHeading>{t('videoPlayer.audioTracks')}</PanelHeading>
      {tracks.map((track) => (
        <PanelOption
          key={track.id}
          label={track.name}
          hint={track.detail}
          selected={!beside && playingTrack === track.id}
          onSelect={() => onTrack(track.id)}
        />
      ))}
      {/* A stream that lists no tracks (a plain file) has its own sound all the same. */}
      {tracks.length === 0 && (
        <PanelOption
          label={t('videoPlayer.audioOwn')}
          selected={!beside}
          onSelect={() => onTrack(-1)}
        />
      )}
      {beside && (
        <PanelOption
          label={
            beside.offer ? offerName(beside.offer) : (beside.file ?? t('videoPlayer.audioFile'))
          }
          hint={
            beside.offer
              ? t('videoPlayer.audioFrom', { n: beside.offer.source })
              : t('videoPlayer.audioFile')
          }
          selected
          onSelect={() => {}}
        />
      )}
      {(session || offers.length > 0) && (
        <>
          <PanelHeading>{t('videoPlayer.audioOtherSources')}</PanelHeading>
          {offers.length === 0 && (
            <p className="px-3 py-2 text-sm text-white/60">
              {t('videoPlayer.audioNoOtherSources')}
            </p>
          )}
          {offers.map((offer) => (
            <PanelOption
              key={offer.id}
              label={offerName(offer)}
              hint={offerDetail(offer)}
              selected={beside?.offer?.id === offer.id}
              onSelect={() => onOffer(offer)}
            />
          ))}
        </>
      )}
    </>
  );
};

/**
 * Add an audio file: a small round button by the Audio panel's title. The file picked plays as
 * the video's sound.
 */
export const AddAudioFileButton: FC<{ onFile: (file: File) => void }> = ({ onFile }) => {
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  return (
    <>
      <button
        type="button"
        aria-label={t('videoPlayer.addAudioFile')}
        onClick={() => input.current?.click()}
        className="grid size-9 place-items-center rounded-full bg-white/10 hover:bg-white/20"
      >
        <FilePlus2 aria-hidden size={18} />
      </button>
      <input
        ref={input}
        type="file"
        accept="audio/*,video/*"
        className="sr-only"
        tabIndex={-1}
        aria-hidden
        data-testid="audio-file-input"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) onFile(file);
        }}
      />
    </>
  );
};
