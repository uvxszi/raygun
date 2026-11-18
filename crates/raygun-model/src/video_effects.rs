//! Real-time effects in the video player: the picture's (light, colour, a look, sharpness,
//! deflicker and denoise) and smart audio (clearer dialogue, night mode, even loudness and scene
//! leveling). Every field at its default leaves the video as it is, so effects are off until
//! changed. A title may keep its own (`pictureEffects` and `smartAudio` in
//! `video.playerState.<group>`); otherwise Settings → Video player → Effects applies
//! (`VideoPlayerPrefs::picture_effects`, `VideoPlayerPrefs::smart_audio`). How the values map onto
//! shaders and audio processing is the players' (`PlayerEffects*.kt`, `video/effects/`).

use serde::{Deserialize, Serialize};
use specta::Type;

/// A colour look laid over the picture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PictureLook {
    #[default]
    None,
    Warm,
    Cool,
    /// Richer colours and a little more contrast.
    Vivid,
    Grayscale,
    Sepia,
}

/// How far the signed adjustments go either way.
pub const PICTURE_ADJUST_RANGE: i32 = 100;
/// How far the hue turns either way, in degrees.
pub const PICTURE_HUE_RANGE: i32 = 180;
/// The most a strength (sharpen, deflicker, denoise) goes to.
pub const PICTURE_STRENGTH_MAX: u32 = 100;

/// The picture's adjustments, each 0 while untouched. Brightness, contrast, saturation and gamma go
/// from −100 to 100, the hue from −180° to 180°, and the strengths from 0 to 100.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct PictureEffects {
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    /// Midtones lighter (above 0) or darker, black and white kept.
    pub gamma: i32,
    /// Degrees the colours turn round the colour wheel.
    pub hue: i32,
    pub sharpen: u32,
    pub look: PictureLook,
    /// Evens out the frame-to-frame brightness changes of old film.
    pub deflicker: u32,
    /// Smooths grain and noise over neighbouring pixels and frames.
    pub denoise: u32,
    /// In-between frames made from the motion between two (about 60 a second, at most the
    /// screen's), so pans and movement look smoother.
    pub smooth_motion: bool,
    /// How smooth motion makes its in-between frames (only while it's on).
    pub motion_model: MotionModel,
    /// A watermark taken out of a box of the picture (per title; off unless turned on).
    pub watermark: WatermarkRemoval,
}

/// Smooth motion's ways of making in-between frames (the Smooth Motion chip's shaders).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MotionModel {
    /// The cheapest: two frames blended where they barely move, the nearer frame repeated
    /// elsewhere.
    Blend,
    /// The motion followed, with occlusions, unsure blocks and the picture's edges handled.
    #[default]
    Balanced,
    /// As balanced, with a finer search and another refinement: sharper on fast detail, slower.
    Sharp,
}

/// The box's sides are in ten-thousandths of the picture's, from its top left.
pub const WATERMARK_SCALE: u32 = 10_000;

/// The watermark remover: a static overlay (a channel's logo, a "sample" mark) inside a box of
/// the picture. Its alpha mask and colour are estimated from the frames as they play (the overlay
/// stays while the video under it changes), then the blend is inverted in the box; parts too
/// opaque to invert are filled from the box's edges. Brightness and contrast correct what's left,
/// the feather softens the box's edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct WatermarkRemoval {
    pub enabled: bool,
    /// The box, in ten-thousandths of the picture ([`WATERMARK_SCALE`]) from its top left.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// −100 to 100, applied where the overlay was.
    pub brightness: i32,
    /// −100 to 100, applied where the overlay was.
    pub contrast: i32,
    /// 0 to 100: how much of the box's edge blends into the picture around it.
    pub feather: u32,
}

impl WatermarkRemoval {
    /// On, with a box to work in.
    pub fn is_active(&self) -> bool {
        self.enabled && self.width > 0 && self.height > 0
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.x + self.width > WATERMARK_SCALE || self.y + self.height > WATERMARK_SCALE {
            return Err("The watermark's box must lie inside the picture".into());
        }
        if self.brightness.abs() > PICTURE_ADJUST_RANGE
            || self.contrast.abs() > PICTURE_ADJUST_RANGE
            || self.feather > PICTURE_STRENGTH_MAX
        {
            return Err(
                "The watermark's corrections go from −100 to 100, its feather to 100".into(),
            );
        }
        Ok(())
    }
}

/// A title's sound in the video player (`videoSound` in `video.playerState.<group>`): pitch, and
/// an equalizer of its own or the music player's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct VideoSound {
    /// Pitch shift in tenths of a semitone (−120 to 120), the tempo kept; the tempo is the
    /// title's speed.
    pub pitch: i32,
    /// The title has an equalizer of its own; otherwise the music player's (`SoundPrefs`) applies.
    pub own_eq: bool,
    pub eq_enabled: bool,
    pub eq_preset: String,
    /// Gains in tenths of a dB (−120 to 120), one per band.
    pub eq_bands: Vec<i32>,
    /// Gain before the equalizer, tenths of a dB.
    pub preamp: i32,
}

impl Default for VideoSound {
    fn default() -> Self {
        Self {
            pitch: 0,
            own_eq: false,
            eq_enabled: false,
            eq_preset: "flat".to_owned(),
            eq_bands: vec![0; crate::EQ_FREQUENCIES.len()],
            preamp: 0,
        }
    }
}

impl PictureEffects {
    /// Nothing changed: the picture plays as it is (a watermark box that's off changes nothing).
    pub fn is_neutral(&self) -> bool {
        // The motion model only matters while smooth motion is on.
        let rest = Self {
            watermark: WatermarkRemoval::default(),
            motion_model: MotionModel::default(),
            ..*self
        };
        rest == Self::default() && !self.watermark.is_active()
    }

    /// Checks the ranges the type can't express.
    pub fn validate(&self) -> Result<(), String> {
        let adjust = -PICTURE_ADJUST_RANGE..=PICTURE_ADJUST_RANGE;
        if [self.brightness, self.contrast, self.saturation, self.gamma]
            .iter()
            .any(|value| !adjust.contains(value))
        {
            return Err("Picture adjustments go from −100 to 100".into());
        }
        if !(-PICTURE_HUE_RANGE..=PICTURE_HUE_RANGE).contains(&self.hue) {
            return Err("The hue turns at most 180° either way".into());
        }
        if [self.sharpen, self.deflicker, self.denoise]
            .iter()
            .any(|value| *value > PICTURE_STRENGTH_MAX)
        {
            return Err("Sharpen, deflicker and denoise go from 0 to 100".into());
        }
        self.watermark.validate()
    }
}

/// Smart audio in the video player, each on or off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct SmartAudio {
    /// Speech brought forward: the voice band lifted, the centre over the sides.
    pub dialogue: bool,
    /// Loud parts quieter and quiet ones louder (dynamic range compression).
    pub night: bool,
    /// Every video at about the same loudness, measured as it plays.
    pub normalize: bool,
    /// Quiet and loud scenes evened out as they come.
    pub leveling: bool,
}

impl SmartAudio {
    /// Nothing on: the sound plays as it is.
    pub fn is_off(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_are_off_until_changed() {
        assert!(PictureEffects::default().is_neutral());
        assert!(SmartAudio::default().is_off());
        let warm = PictureEffects {
            look: PictureLook::Warm,
            ..PictureEffects::default()
        };
        assert!(!warm.is_neutral());
        // A motion model chosen changes nothing while smooth motion is off.
        let sharp = PictureEffects {
            motion_model: MotionModel::Sharp,
            ..PictureEffects::default()
        };
        assert!(sharp.is_neutral());
        let parsed: PictureEffects =
            serde_json::from_str(r#"{"smoothMotion": true, "motionModel": "blend"}"#).unwrap();
        assert_eq!(parsed.motion_model, MotionModel::Blend);
        assert!(!parsed.is_neutral());
    }

    #[test]
    fn missing_fields_keep_their_defaults() {
        let effects: PictureEffects =
            serde_json::from_str(r#"{"deflicker": 40, "look": "sepia"}"#).unwrap();
        assert_eq!(
            effects,
            PictureEffects {
                deflicker: 40,
                look: PictureLook::Sepia,
                ..PictureEffects::default()
            }
        );
        let audio: SmartAudio = serde_json::from_str(r#"{"night": true}"#).unwrap();
        assert!(audio.night && !audio.dialogue && !audio.normalize && !audio.leveling);
        let json = serde_json::to_value(PictureEffects::default()).unwrap();
        assert_eq!(json["look"], "none");
        assert_eq!(json["smoothMotion"], false);
        assert_eq!(json["deflicker"], 0);
    }

    #[test]
    fn checks_the_ranges() {
        let mut effects = PictureEffects {
            brightness: -100,
            contrast: 100,
            hue: 180,
            sharpen: 100,
            ..PictureEffects::default()
        };
        assert!(effects.validate().is_ok());
        effects.saturation = 101;
        assert!(effects.validate().is_err());
        effects.saturation = 0;
        effects.hue = -181;
        assert!(effects.validate().is_err());
        effects.hue = 0;
        effects.denoise = 101;
        assert!(effects.validate().is_err());
        effects.denoise = 0;
        effects.watermark = WatermarkRemoval {
            enabled: true,
            x: 8_000,
            y: 500,
            width: 2_500,
            height: 1_000,
            ..WatermarkRemoval::default()
        };
        assert!(effects.validate().is_err());
        effects.watermark.width = 1_500;
        assert!(effects.validate().is_ok());
    }

    #[test]
    fn a_watermark_box_left_off_changes_nothing() {
        let mut effects = PictureEffects {
            watermark: WatermarkRemoval {
                x: 100,
                y: 100,
                width: 1_000,
                height: 500,
                ..WatermarkRemoval::default()
            },
            ..PictureEffects::default()
        };
        assert!(effects.is_neutral());
        effects.watermark.enabled = true;
        assert!(!effects.is_neutral());
        let json = serde_json::to_value(effects).unwrap();
        assert_eq!(json["watermark"]["width"], 1_000);
        let sound: VideoSound = serde_json::from_str(r#"{"pitch": -20}"#).unwrap();
        assert_eq!(sound.pitch, -20);
        assert_eq!(sound.eq_bands.len(), crate::EQ_FREQUENCIES.len());
        assert!(!sound.own_eq);
    }
}
