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
}

impl PictureEffects {
    /// Nothing changed: the picture plays as it is.
    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
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
        Ok(())
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
    }
}
