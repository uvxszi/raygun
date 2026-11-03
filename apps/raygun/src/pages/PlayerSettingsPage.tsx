import type { SubtitleStyle } from '@raygun/bindings';
import { useParams } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LANGUAGES, nativeName } from '../i18n/languages';
import {
  ASPECTS,
  BACKGROUND_COLORS,
  CONTROLS_SECONDS,
  DECODERS,
  DEFAULT_SUBTITLE_STYLE,
  EDGE_COLORS,
  EXTERNAL_PLAYERS,
  formatSpeed,
  HOLD_SPEEDS,
  isPlayerPage,
  MEDIA_KEY_ACTIONS,
  ORIENTATIONS,
  parseSpeeds,
  SKIP_WAIT_SECONDS,
  SUBTITLE_EDGES,
  SUBTITLE_FONTS,
  TAP_ACTIONS,
  TEXT_COLORS,
  withDefaults,
} from '../settings/playerOptions';
import type { PlayerPrefs, PlayerSettingsPage as Page } from '../settings/playerOptions';
import { PrefChoice, PrefGroup, PrefSlider, PrefSwitch, PrefText } from '../settings/prefs';
import { SubtitlePreview } from '../settings/SubtitlePreview';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { TopBar } from '../shell/TopBar';

/** Matches `SEEK_SECONDS` in raygun-model. */
const SEEK_SECONDS = [5, 10, 15, 30, 60];

/** The player's settings, changed a few fields at a time. */
const usePlayerPrefs = () => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const prefs = settings.data ? withDefaults(settings.data.videoPlayer) : undefined;
  const set = (change: Partial<PlayerPrefs>) => {
    if (prefs) update.mutate({ key: 'videoPlayer', value: { ...prefs, ...change } });
  };
  return { settings: settings.data, prefs, set, update, busy: update.isPending };
};

type ScreenProps = {
  prefs: PlayerPrefs;
  set: (change: Partial<PlayerPrefs>) => void;
  busy: boolean;
};

const languageOptions = (none: string) => [
  { value: '', label: none },
  ...LANGUAGES.map((tag) => ({ value: tag, label: nativeName(tag) })),
];

const ControlsScreen: FC<ScreenProps> = ({ prefs, set, busy }) => {
  const { t } = useTranslation();
  return (
    <>
      <PrefGroup title={t('settings.player.display')}>
        <PrefChoice
          title={t('settings.player.orientation')}
          summary={t('settings.player.orientationHint')}
          value={prefs.orientation}
          disabled={busy}
          options={ORIENTATIONS.map((value) => ({
            value,
            label: t(`settings.player.orientations.${value}`),
          }))}
          onChange={(orientation) => set({ orientation })}
        />
        <PrefChoice
          title={t('settings.player.aspect')}
          summary={t('settings.player.aspectHint')}
          value={prefs.aspect}
          disabled={busy}
          options={ASPECTS.map((value) => ({
            value,
            label: t(`settings.player.aspects.${value}`),
          }))}
          onChange={(aspect) => set({ aspect })}
        />
        <PrefChoice
          title={t('settings.player.controlsTimeout')}
          value={prefs.controlsSeconds}
          disabled={busy}
          options={CONTROLS_SECONDS.map((seconds) => ({
            value: seconds,
            label: t('settings.seconds', { count: seconds }),
          }))}
          onChange={(controlsSeconds) => set({ controlsSeconds })}
        />
        <PrefSwitch
          title={t('settings.player.showRemaining')}
          summary={t('settings.player.showRemainingHint')}
          checked={prefs.showRemaining}
          disabled={busy}
          onChange={(showRemaining) => set({ showRemaining })}
        />
        <PrefSwitch
          title={t('settings.player.rememberBrightness')}
          summary={t('settings.player.rememberBrightnessHint')}
          checked={prefs.rememberBrightness}
          disabled={busy}
          onChange={(rememberBrightness) => set({ rememberBrightness })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.speed')}>
        <PrefSlider
          title={t('settings.player.defaultSpeed')}
          summary={t('settings.player.defaultSpeedHint')}
          value={prefs.defaultSpeed}
          min={25}
          max={400}
          step={5}
          format={formatSpeed}
          disabled={busy}
          onChange={(defaultSpeed) => set({ defaultSpeed })}
        />
        <PrefText
          title={t('settings.player.speedPresets')}
          summary={t('settings.player.speedPresetsHint')}
          value={prefs.speedPresets.map((speed) => speed / 100).join(', ')}
          check={(text) => (parseSpeeds(text) ? null : t('settings.player.speedPresetsInvalid'))}
          disabled={busy}
          onChange={(text) => {
            const speedPresets = parseSpeeds(text);
            if (speedPresets) set({ speedPresets });
          }}
        />
        <PrefChoice
          title={t('settings.player.holdSpeed')}
          summary={t('settings.player.holdSpeedHint')}
          value={prefs.holdSpeed}
          disabled={busy}
          options={HOLD_SPEEDS.map((speed) => ({
            value: speed,
            label: speed === 0 ? t('settings.player.off') : formatSpeed(speed),
          }))}
          onChange={(holdSpeed) => set({ holdSpeed })}
        />
      </PrefGroup>
    </>
  );
};

const GesturesScreen: FC<ScreenProps & { seekSeconds: number }> = ({
  prefs,
  set,
  busy,
  seekSeconds,
}) => {
  const { t } = useTranslation();
  const update = useUpdateSetting();
  const tapOptions = TAP_ACTIONS.map((value) => ({
    value,
    label: t(`settings.player.tapActions.${value}`),
  }));
  return (
    <>
      <PrefGroup title={t('settings.player.swipes')}>
        <PrefSwitch
          title={t('settings.player.brightnessGesture')}
          summary={t(prefs.swapSides ? 'settings.player.onTheRight' : 'settings.player.onTheLeft')}
          checked={prefs.brightnessGesture}
          disabled={busy}
          onChange={(brightnessGesture) => set({ brightnessGesture })}
        />
        <PrefSwitch
          title={t('settings.player.volumeGesture')}
          summary={t(prefs.swapSides ? 'settings.player.onTheLeft' : 'settings.player.onTheRight')}
          checked={prefs.volumeGesture}
          disabled={busy}
          onChange={(volumeGesture) => set({ volumeGesture })}
        />
        <PrefSwitch
          title={t('settings.player.swapSides')}
          summary={t('settings.player.swapSidesHint')}
          checked={prefs.swapSides}
          disabled={busy}
          onChange={(swapSides) => set({ swapSides })}
        />
        <PrefSwitch
          title={t('settings.player.seekGesture')}
          summary={t('settings.player.seekGestureHint')}
          checked={prefs.seekGesture}
          disabled={busy}
          onChange={(seekGesture) => set({ seekGesture })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.doubleTap')}>
        <PrefChoice
          title={t('settings.player.doubleTapLeft')}
          value={prefs.doubleTapLeft}
          disabled={busy}
          options={tapOptions}
          onChange={(doubleTapLeft) => set({ doubleTapLeft })}
        />
        <PrefChoice
          title={t('settings.player.doubleTapCenter')}
          value={prefs.doubleTapCenter}
          disabled={busy}
          options={tapOptions.filter((option) => option.value !== 'seek')}
          onChange={(doubleTapCenter) => set({ doubleTapCenter })}
        />
        <PrefChoice
          title={t('settings.player.doubleTapRight')}
          value={prefs.doubleTapRight}
          disabled={busy}
          options={tapOptions}
          onChange={(doubleTapRight) => set({ doubleTapRight })}
        />
        <PrefChoice<number>
          title={t('settings.seekSeconds')}
          summary={t('settings.seekSecondsHint')}
          value={seekSeconds}
          disabled={busy || update.isPending}
          options={SEEK_SECONDS.map((seconds) => ({
            value: seconds,
            label: t('settings.seconds', { count: seconds }),
          }))}
          onChange={(value) => update.mutate({ key: 'seekSeconds', value })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.keys')}>
        <PrefChoice
          title={t('settings.player.mediaKeys')}
          summary={t('settings.player.mediaKeysHint')}
          value={prefs.mediaKeys}
          disabled={busy}
          options={MEDIA_KEY_ACTIONS.map((value) => ({
            value,
            label: t(`settings.player.mediaKeyActions.${value}`),
          }))}
          onChange={(mediaKeys) => set({ mediaKeys })}
        />
      </PrefGroup>
    </>
  );
};

const colorOptions = (colors: { value: number; name: string }[], t: (key: string) => string) =>
  colors.map((color) => ({ value: color.value, label: t(`settings.player.colors.${color.name}`) }));

const SubtitlesScreen: FC<ScreenProps & { autoSubtitles: boolean; subtitleLanguage: string }> = ({
  prefs,
  set,
  busy,
  autoSubtitles,
  subtitleLanguage,
}) => {
  const { t } = useTranslation();
  const update = useUpdateSetting();
  const style = prefs.subtitleStyle;
  const setStyle = (change: Partial<SubtitleStyle>) =>
    set({ subtitleStyle: { ...style, ...change } });
  return (
    <>
      <PrefGroup title={t('settings.player.choosing')}>
        <PrefSwitch
          title={t('settings.autoSubtitles')}
          summary={t('settings.autoSubtitlesHint')}
          checked={autoSubtitles}
          disabled={busy || update.isPending}
          onChange={(value) => update.mutate({ key: 'autoSubtitles', value })}
        />
        <PrefChoice<string>
          title={t('settings.subtitleLanguage')}
          summary={t('settings.subtitleLanguageHint')}
          value={subtitleLanguage}
          disabled={busy || update.isPending}
          options={languageOptions(t('settings.subtitleLanguageApp'))}
          onChange={(value) => update.mutate({ key: 'subtitleLanguage', value })}
        />
        <PrefChoice<string>
          title={t('settings.player.subtitleFallback')}
          summary={t('settings.player.subtitleFallbackHint')}
          value={prefs.subtitleFallback}
          disabled={busy}
          options={languageOptions(t('settings.player.none'))}
          onChange={(subtitleFallback) => set({ subtitleFallback })}
        />
        <PrefText
          title={t('settings.player.preferWords')}
          summary={t('settings.player.preferWordsHint')}
          value={prefs.subtitlePreferWords}
          placeholder={t('settings.player.noWords')}
          check={(text) => (text.length > 200 ? t('settings.player.tooLong') : null)}
          disabled={busy}
          onChange={(subtitlePreferWords) => set({ subtitlePreferWords })}
        />
        <PrefText
          title={t('settings.player.skipWords')}
          summary={t('settings.player.skipWordsHint')}
          value={prefs.subtitleSkipWords}
          placeholder={t('settings.player.noWords')}
          check={(text) => (text.length > 200 ? t('settings.player.tooLong') : null)}
          disabled={busy}
          onChange={(subtitleSkipWords) => set({ subtitleSkipWords })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.style')}>
        <li className="px-1 py-2">
          <SubtitlePreview style={style} />
        </li>
        <PrefSlider
          title={t('settings.player.subtitleSize')}
          value={style.size}
          min={50}
          max={200}
          step={10}
          format={(value) => `${value}%`}
          disabled={busy}
          onChange={(size) => setStyle({ size })}
        />
        <PrefChoice
          title={t('settings.player.subtitleFont')}
          value={style.font}
          disabled={busy}
          options={SUBTITLE_FONTS.map((value) => ({
            value,
            label: t(`settings.player.fonts.${value}`),
          }))}
          onChange={(font) => setStyle({ font })}
        />
        <PrefSwitch
          title={t('settings.player.bold')}
          checked={style.bold}
          disabled={busy}
          onChange={(bold) => setStyle({ bold })}
        />
        <PrefSwitch
          title={t('settings.player.italic')}
          checked={style.italic}
          disabled={busy}
          onChange={(italic) => setStyle({ italic })}
        />
        <PrefChoice
          title={t('settings.player.textColor')}
          value={style.textColor}
          disabled={busy}
          options={colorOptions(TEXT_COLORS, t)}
          onChange={(textColor) => setStyle({ textColor })}
        />
        <PrefChoice
          title={t('settings.player.edge')}
          value={style.edge}
          disabled={busy}
          options={SUBTITLE_EDGES.map((value) => ({
            value,
            label: t(`settings.player.edges.${value}`),
          }))}
          onChange={(edge) => setStyle({ edge })}
        />
        <PrefChoice
          title={t('settings.player.edgeColor')}
          value={style.edgeColor}
          disabled={busy || style.edge === 'none'}
          options={colorOptions(EDGE_COLORS, t)}
          onChange={(edgeColor) => setStyle({ edgeColor })}
        />
        <PrefChoice
          title={t('settings.player.backgroundColor')}
          value={style.backgroundColor}
          disabled={busy}
          options={colorOptions(BACKGROUND_COLORS, t)}
          onChange={(backgroundColor) => setStyle({ backgroundColor })}
        />
        <PrefSlider
          title={t('settings.player.subtitlePosition')}
          value={style.position}
          min={0}
          max={40}
          step={2}
          format={(value) => `${value}%`}
          disabled={busy}
          onChange={(position) => setStyle({ position })}
        />
        <PrefSwitch
          title={t('settings.player.ignoreEmbedded')}
          summary={t('settings.player.ignoreEmbeddedHint')}
          checked={style.ignoreEmbedded}
          disabled={busy}
          onChange={(ignoreEmbedded) => setStyle({ ignoreEmbedded })}
        />
        <li>
          <button
            type="button"
            disabled={busy}
            onClick={() => set({ subtitleStyle: DEFAULT_SUBTITLE_STYLE })}
            className="text-primary hover:bg-surface w-full rounded-xl px-1 py-3 text-left font-medium disabled:opacity-60"
          >
            {t('settings.player.resetStyle')}
          </button>
        </li>
      </PrefGroup>
    </>
  );
};

const AudioScreen: FC<ScreenProps> = ({ prefs, set, busy }) => {
  const { t } = useTranslation();
  return (
    <PrefGroup>
      <PrefChoice<string>
        title={t('settings.player.audioLanguage')}
        summary={t('settings.player.audioLanguageHint')}
        value={prefs.audioLanguage}
        disabled={busy}
        options={languageOptions(t('settings.player.audioOriginal'))}
        onChange={(audioLanguage) => set({ audioLanguage })}
      />
      <PrefChoice<string>
        title={t('settings.player.audioFallback')}
        summary={t('settings.player.audioFallbackHint')}
        value={prefs.audioFallback}
        disabled={busy || prefs.audioLanguage === ''}
        options={languageOptions(t('settings.player.none'))}
        onChange={(audioFallback) => set({ audioFallback })}
      />
      <PrefSwitch
        title={t('settings.player.pitchCorrection')}
        summary={t('settings.player.pitchCorrectionHint')}
        checked={prefs.pitchCorrection}
        disabled={busy}
        onChange={(pitchCorrection) => set({ pitchCorrection })}
      />
      <PrefSlider
        title={t('settings.player.volumeBoost')}
        summary={t('settings.player.volumeBoostHint')}
        value={prefs.volumeBoost}
        min={0}
        max={100}
        step={10}
        format={(value) => (value === 0 ? t('settings.player.off') : `+${value}%`)}
        disabled={busy}
        onChange={(volumeBoost) => set({ volumeBoost })}
      />
    </PrefGroup>
  );
};

const SkipScreen: FC<ScreenProps> = ({ prefs, set, busy }) => {
  const { t } = useTranslation();
  return (
    <>
      <PrefGroup>
        <PrefSwitch
          title={t('settings.player.skipButton')}
          summary={t('settings.player.skipButtonHint')}
          checked={prefs.skipButton}
          disabled={busy}
          onChange={(skipButton) => set({ skipButton })}
        />
        <PrefSlider
          title={t('settings.player.introSeconds')}
          summary={t('settings.player.introSecondsHint')}
          value={prefs.introSeconds}
          min={5}
          max={300}
          step={5}
          format={(value) => t('settings.seconds', { count: value })}
          disabled={busy}
          onChange={(introSeconds) => set({ introSeconds })}
        />
        <PrefSwitch
          title={t('settings.player.autoSkip')}
          summary={t('settings.player.autoSkipHint')}
          checked={prefs.autoSkip}
          disabled={busy}
          onChange={(autoSkip) => set({ autoSkip })}
        />
        <PrefSwitch
          title={t('settings.player.skipCountdown')}
          summary={t('settings.player.skipCountdownHint')}
          checked={prefs.skipCountdown}
          disabled={busy}
          onChange={(skipCountdown) => set({ skipCountdown })}
        />
        <PrefChoice
          title={t('settings.player.skipWait')}
          value={prefs.skipWaitSeconds}
          disabled={busy || !prefs.skipCountdown}
          options={SKIP_WAIT_SECONDS.map((seconds) => ({
            value: seconds,
            label: t('settings.seconds', { count: seconds }),
          }))}
          onChange={(skipWaitSeconds) => set({ skipWaitSeconds })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.aniskipGroup')}>
        <PrefSwitch
          title={t('settings.player.aniskip')}
          summary={t('settings.player.aniskipHint')}
          checked={prefs.aniskip}
          disabled={busy}
          onChange={(aniskip) => set({ aniskip })}
        />
        <PrefSwitch
          title={t('settings.player.aniskipUnlessChapters')}
          summary={t('settings.player.aniskipUnlessChaptersHint')}
          checked={prefs.aniskipUnlessChapters}
          disabled={busy || !prefs.aniskip}
          onChange={(aniskipUnlessChapters) => set({ aniskipUnlessChapters })}
        />
      </PrefGroup>
    </>
  );
};

const PipScreen: FC<ScreenProps> = ({ prefs, set, busy }) => {
  const { t } = useTranslation();
  return (
    <PrefGroup>
      <PrefSwitch
        title={t('settings.player.pipOnLeave')}
        summary={t('settings.player.pipOnLeaveHint')}
        checked={prefs.pipOnLeave}
        disabled={busy}
        onChange={(pipOnLeave) => set({ pipOnLeave })}
      />
      <PrefSwitch
        title={t('settings.player.pipOnBack')}
        summary={t('settings.player.pipOnBackHint')}
        checked={prefs.pipOnBack}
        disabled={busy}
        onChange={(pipOnBack) => set({ pipOnBack })}
      />
    </PrefGroup>
  );
};

const AdvancedScreen: FC<ScreenProps> = ({ prefs, set, busy }) => {
  const { t } = useTranslation();
  return (
    <>
      <PrefGroup title={t('settings.player.decoding')}>
        <PrefChoice
          title={t('settings.player.decoder')}
          summary={t('settings.player.decoderHint')}
          value={prefs.decoder}
          disabled={busy}
          options={DECODERS.map((value) => ({
            value,
            label: t(`settings.player.decoders.${value}`),
          }))}
          onChange={(decoder) => set({ decoder })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.titlePages')}>
        <PrefSwitch
          title={t('settings.player.titleTrailers')}
          summary={t('settings.player.titleTrailersHint')}
          checked={prefs.titleTrailers}
          disabled={busy}
          onChange={(titleTrailers) => set({ titleTrailers })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.player.otherApps')}>
        <PrefSwitch
          title={t('settings.player.externalPlayer')}
          summary={t('settings.player.externalPlayerHint')}
          checked={prefs.externalPlayer}
          disabled={busy}
          onChange={(externalPlayer) => set({ externalPlayer })}
        />
        <PrefChoice<string>
          title={t('settings.player.externalPackage')}
          summary={t('settings.player.externalPackageHint')}
          value={prefs.externalPackage}
          disabled={busy}
          options={[{ value: '', label: t('settings.player.externalAsk') }, ...EXTERNAL_PLAYERS]}
          onChange={(externalPackage) => set({ externalPackage })}
        />
      </PrefGroup>
    </>
  );
};

/** Settings → Player → one of its screens (`/settings/player/<page>`). */
export const PlayerSettingsPage: FC = () => {
  const { t } = useTranslation();
  const { page } = useParams({ strict: false });
  const { settings, prefs, set, update, busy } = usePlayerPrefs();
  const current: Page = isPlayerPage(page) ? page : 'controls';
  const props = prefs ? { prefs, set, busy } : null;
  return (
    <section data-testid={`player-settings-${current}`} className="flex max-w-2xl flex-col">
      <TopBar
        back
        fallback="/settings/player"
        title={t(`settings.player.section.${current}.title`)}
      />
      {props && settings && (
        <>
          {current === 'controls' && <ControlsScreen {...props} />}
          {current === 'gestures' && (
            <GesturesScreen {...props} seekSeconds={settings.seekSeconds} />
          )}
          {current === 'subtitles' && (
            <SubtitlesScreen
              {...props}
              autoSubtitles={settings.autoSubtitles}
              subtitleLanguage={settings.subtitleLanguage}
            />
          )}
          {current === 'audio' && <AudioScreen {...props} />}
          {current === 'skip' && <SkipScreen {...props} />}
          {current === 'pip' && <PipScreen {...props} />}
          {current === 'advanced' && <AdvancedScreen {...props} />}
        </>
      )}
      {update.isError && (
        <p role="alert" className="text-primary px-1 text-sm">
          {t('settings.saveFailed', { message: update.error.message })}
        </p>
      )}
    </section>
  );
};
