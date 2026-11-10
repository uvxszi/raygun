//! Elementary streams. H.264 and HEVC arrive in MPEG-TS as Annex B (start codes) and leave as
//! length-prefixed samples with their configuration records (`avcC`, `hvcC`); AAC's ADTS headers
//! become an AudioSpecificConfig; MPEG audio, AC-3 and E-AC-3 frames are found by their headers.

use mp4_atom::{Atom, Avcc, AvccExt, Encode, HvcCArray, Hvcc};

/// Reads bits, most significant first, from an RBSP (emulation prevention bytes removed).
pub struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn bit(&mut self) -> Option<bool> {
        let byte = *self.data.get(self.pos / 8)?;
        let bit = byte >> (7 - self.pos % 8) & 1;
        self.pos += 1;
        Some(bit == 1)
    }

    pub fn bits(&mut self, count: u32) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..count {
            value = value << 1 | u32::from(self.bit()?);
        }
        Some(value)
    }

    pub fn skip(&mut self, count: usize) -> Option<()> {
        self.pos += count;
        (self.pos <= self.data.len() * 8).then_some(())
    }

    /// Exp-Golomb, unsigned.
    pub fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while !self.bit()? {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        Some(((1u64 << zeros) - 1 + u64::from(self.bits(zeros)?)) as u32)
    }

    /// Exp-Golomb, signed.
    pub fn se(&mut self) -> Option<i32> {
        let code = self.ue()?;
        Some(if code % 2 == 1 {
            code.div_ceil(2) as i32
        } else {
            -((code / 2) as i32)
        })
    }
}

/// A NAL unit without its emulation prevention bytes (`00 00 03` becomes `00 00`).
pub fn unescape(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &byte in nal {
        if zeros >= 2 && byte == 3 {
            zeros = 0;
            continue;
        }
        zeros = if byte == 0 { zeros + 1 } else { 0 };
        out.push(byte);
    }
    out
}

/// The NAL units of Annex B data, without their start codes.
pub fn nal_units(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut index = 0;
    while index + 3 <= data.len() {
        if data[index] == 0 && data[index + 1] == 0 && data[index + 2] == 1 {
            starts.push(index + 3);
            index += 3;
        } else {
            index += 1;
        }
    }
    let mut units = Vec::with_capacity(starts.len());
    for (at, &start) in starts.iter().enumerate() {
        let mut end = starts.get(at + 1).map_or(data.len(), |next| next - 3);
        // The zero of a four-byte start code belongs to the next one.
        while end > start && data[end - 1] == 0 && at + 1 < starts.len() {
            end -= 1;
        }
        if end > start {
            units.push(&data[start..end]);
        }
    }
    units
}

/// Length-prefixed samples (four bytes, big-endian) from NAL units.
pub fn length_prefixed<'a>(units: impl IntoIterator<Item = &'a [u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    for unit in units {
        out.extend_from_slice(&(unit.len() as u32).to_be_bytes());
        out.extend_from_slice(unit);
    }
    out
}

/// The body of an atom (its record), without the box header: what Matroska's CodecPrivate holds.
pub fn atom_body(atom: &impl Atom) -> Vec<u8> {
    let mut buf = Vec::new();
    // Encoding into memory can't fail for these records.
    let _ = atom.encode(&mut buf);
    buf.split_off(8.min(buf.len()))
}

/// What an H.264 SPS says about the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvcSps {
    pub profile: u8,
    pub chroma_format: u32,
    pub bit_depth_luma: u32,
    pub bit_depth_chroma: u32,
    pub width: u32,
    pub height: u32,
}

/// Reads an H.264 SPS NAL unit (with its one-byte header).
pub fn parse_avc_sps(nal: &[u8]) -> Option<AvcSps> {
    let rbsp = unescape(nal.get(1..)?);
    let mut bits = Bits::new(&rbsp);
    let profile = bits.bits(8)? as u8;
    bits.skip(16)?;
    bits.ue()?;
    let (mut chroma_format, mut depth_luma, mut depth_chroma, mut separate) = (1, 8, 8, false);
    if matches!(
        profile,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        chroma_format = bits.ue()?;
        if chroma_format == 3 {
            separate = bits.bit()?;
        }
        depth_luma = bits.ue()? + 8;
        depth_chroma = bits.ue()? + 8;
        bits.bit()?;
        if bits.bit()? {
            let lists = if chroma_format == 3 { 12 } else { 8 };
            for list in 0..lists {
                if bits.bit()? {
                    let size = if list < 6 { 16 } else { 64 };
                    let (mut last, mut next) = (8i32, 8i32);
                    for _ in 0..size {
                        if next != 0 {
                            next = (last + bits.se()? + 256) % 256;
                        }
                        if next != 0 {
                            last = next;
                        }
                    }
                }
            }
        }
    }
    bits.ue()?;
    match bits.ue()? {
        0 => {
            bits.ue()?;
        }
        1 => {
            bits.bit()?;
            bits.se()?;
            bits.se()?;
            for _ in 0..bits.ue()? {
                bits.se()?;
            }
        }
        _ => {}
    }
    bits.ue()?;
    bits.bit()?;
    let width_mbs = bits.ue()? + 1;
    let height_units = bits.ue()? + 1;
    let frame_mbs_only = bits.bit()?;
    if !frame_mbs_only {
        bits.bit()?;
    }
    bits.bit()?;
    let (mut left, mut right, mut top, mut bottom) = (0, 0, 0, 0);
    if bits.bit()? {
        left = bits.ue()?;
        right = bits.ue()?;
        top = bits.ue()?;
        bottom = bits.ue()?;
    }
    let field_factor = if frame_mbs_only { 1 } else { 2 };
    let array_type = if separate { 0 } else { chroma_format };
    let (unit_x, unit_y) = match array_type {
        0 => (1, field_factor),
        1 => (2, 2 * field_factor),
        2 => (2, field_factor),
        _ => (1, field_factor),
    };
    let width = (width_mbs * 16).saturating_sub(unit_x * (left + right));
    let height = (field_factor * height_units * 16).saturating_sub(unit_y * (top + bottom));
    Some(AvcSps {
        profile,
        chroma_format,
        bit_depth_luma: depth_luma,
        bit_depth_chroma: depth_chroma,
        width,
        height,
    })
}

/// The `avcC` record of a stream from its SPS and PPS.
pub fn avc_config(sps: &[u8], pps: &[u8]) -> Option<(Avcc, AvcSps)> {
    let info = parse_avc_sps(sps)?;
    let mut avcc = Avcc::new(sps, pps).ok()?;
    if matches!(info.profile, 100 | 110 | 122 | 144) {
        avcc.ext = Some(AvccExt {
            chroma_format: info.chroma_format as u8,
            bit_depth_luma: info.bit_depth_luma as u8,
            bit_depth_chroma: info.bit_depth_chroma as u8,
            sequence_parameter_sets_ext: Vec::new(),
        });
    }
    Some((avcc, info))
}

/// The H.264 NAL unit type.
pub fn avc_type(nal: &[u8]) -> u8 {
    nal.first().map_or(0, |byte| byte & 0x1F)
}

pub const AVC_IDR: u8 = 5;
pub const AVC_SEI: u8 = 6;
pub const AVC_SPS: u8 = 7;
pub const AVC_PPS: u8 = 8;
pub const AVC_AUD: u8 = 9;
pub const AVC_FILLER: u8 = 12;

/// Whether an access unit starts a picture a decoder can begin from: an IDR, or an I slice with a
/// recovery point (open GOPs).
pub fn avc_random_access(units: &[&[u8]]) -> bool {
    if units.iter().any(|unit| avc_type(unit) == AVC_IDR) {
        return true;
    }
    let recovery = units
        .iter()
        .any(|unit| avc_type(unit) == AVC_SEI && sei_has(unit, 1, 6));
    recovery
        && units
            .iter()
            .find(|unit| avc_type(unit) == 1)
            .is_some_and(|slice| {
                let rbsp = unescape(&slice[1..slice.len().min(16)]);
                let mut bits = Bits::new(&rbsp);
                bits.ue().is_some() && bits.ue().is_some_and(|kind| kind % 5 == 2)
            })
}

/// Whether an SEI NAL unit (header `header` bytes long) carries a message of `kind`.
fn sei_has(unit: &[u8], header: usize, kind: u32) -> bool {
    let rbsp = unescape(unit.get(header..).unwrap_or_default());
    let mut at = 0;
    while at < rbsp.len() && rbsp[at] != 0x80 {
        let mut payload = 0u32;
        while *rbsp.get(at).unwrap_or(&0) == 0xFF {
            payload += 255;
            at += 1;
        }
        payload += u32::from(*rbsp.get(at).unwrap_or(&0));
        at += 1;
        let mut size = 0usize;
        while *rbsp.get(at).unwrap_or(&0) == 0xFF {
            size += 255;
            at += 1;
        }
        size += usize::from(*rbsp.get(at).unwrap_or(&0));
        at += 1;
        if payload == kind {
            return true;
        }
        at += size;
    }
    false
}

/// The HEVC NAL unit type.
pub fn hevc_type(nal: &[u8]) -> u8 {
    nal.first().map_or(0, |byte| byte >> 1 & 0x3F)
}

pub const HEVC_VPS: u8 = 32;
pub const HEVC_SPS: u8 = 33;
pub const HEVC_PPS: u8 = 34;
pub const HEVC_AUD: u8 = 35;
pub const HEVC_FILLER: u8 = 38;

/// BLA, IDR and CRA pictures: a decoder can start there.
pub fn hevc_random_access(units: &[&[u8]]) -> bool {
    units
        .iter()
        .any(|unit| (16..=21).contains(&hevc_type(unit)))
}

/// What an HEVC SPS says, for `hvcC` and the picture's size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HevcSps {
    pub profile_space: u8,
    pub tier: bool,
    pub profile_idc: u8,
    pub compatibility: [u8; 4],
    pub constraints: [u8; 6],
    pub level_idc: u8,
    pub sub_layers: u8,
    pub temporal_id_nested: bool,
    pub chroma_format: u8,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub width: u32,
    pub height: u32,
}

/// Reads an HEVC SPS NAL unit (with its two-byte header).
pub fn parse_hevc_sps(nal: &[u8]) -> Option<HevcSps> {
    let rbsp = unescape(nal.get(2..)?);
    let mut bits = Bits::new(&rbsp);
    bits.bits(4)?;
    let max_sub_layers_minus1 = bits.bits(3)? as usize;
    let temporal_id_nested = bits.bit()?;
    let profile_space = bits.bits(2)? as u8;
    let tier = bits.bit()?;
    let profile_idc = bits.bits(5)? as u8;
    let mut compatibility = [0u8; 4];
    for byte in &mut compatibility {
        *byte = bits.bits(8)? as u8;
    }
    let mut constraints = [0u8; 6];
    for byte in &mut constraints {
        *byte = bits.bits(8)? as u8;
    }
    let level_idc = bits.bits(8)? as u8;
    let mut present = Vec::with_capacity(max_sub_layers_minus1);
    for _ in 0..max_sub_layers_minus1 {
        present.push((bits.bit()?, bits.bit()?));
    }
    if max_sub_layers_minus1 > 0 {
        for _ in max_sub_layers_minus1..8 {
            bits.bits(2)?;
        }
    }
    for (profile, level) in present {
        if profile {
            bits.skip(88)?;
        }
        if level {
            bits.skip(8)?;
        }
    }
    bits.ue()?;
    let chroma_format = bits.ue()?;
    if chroma_format == 3 {
        bits.bit()?;
    }
    let width = bits.ue()?;
    let height = bits.ue()?;
    let (mut left, mut right, mut top, mut bottom) = (0, 0, 0, 0);
    if bits.bit()? {
        left = bits.ue()?;
        right = bits.ue()?;
        top = bits.ue()?;
        bottom = bits.ue()?;
    }
    let bit_depth_luma_minus8 = bits.ue()? as u8;
    let bit_depth_chroma_minus8 = bits.ue()? as u8;
    let (sub_width, sub_height) = match chroma_format {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    Some(HevcSps {
        profile_space,
        tier,
        profile_idc,
        compatibility,
        constraints,
        level_idc,
        sub_layers: max_sub_layers_minus1 as u8 + 1,
        temporal_id_nested,
        chroma_format: chroma_format as u8,
        bit_depth_luma_minus8,
        bit_depth_chroma_minus8,
        width: width.saturating_sub(sub_width * (left + right)),
        height: height.saturating_sub(sub_height * (top + bottom)),
    })
}

/// The `hvcC` record of a stream from its parameter sets.
pub fn hevc_config(vps: &[u8], sps: &[u8], pps: &[u8]) -> Option<(Hvcc, HevcSps)> {
    let info = parse_hevc_sps(sps)?;
    let array = |nal_unit_type: u8, nal: &[u8]| HvcCArray {
        completeness: true,
        nal_unit_type,
        nalus: vec![nal.to_vec()],
    };
    let hvcc = Hvcc {
        configuration_version: 1,
        general_profile_space: info.profile_space,
        general_tier_flag: info.tier,
        general_profile_idc: info.profile_idc,
        general_profile_compatibility_flags: info.compatibility,
        general_constraint_indicator_flags: info.constraints,
        general_level_idc: info.level_idc,
        min_spatial_segmentation_idc: 0,
        parallelism_type: 0,
        chroma_format_idc: info.chroma_format,
        bit_depth_luma_minus8: info.bit_depth_luma_minus8,
        bit_depth_chroma_minus8: info.bit_depth_chroma_minus8,
        avg_frame_rate: 0,
        constant_frame_rate: 0,
        num_temporal_layers: info.sub_layers,
        temporal_id_nested: info.temporal_id_nested,
        length_size_minus_one: 3,
        arrays: vec![
            array(HEVC_VPS, vps),
            array(HEVC_SPS, sps),
            array(HEVC_PPS, pps),
        ],
    };
    Some((hvcc, info))
}

/// AAC sampling rates by their index.
pub const AAC_RATES: [u32; 13] = [
    96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000,
    7_350,
];

/// One ADTS frame's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Adts {
    /// MPEG-4 audio object type (2 is AAC LC).
    pub object_type: u8,
    pub rate_index: u8,
    pub channels: u8,
    /// The header's length (7, or 9 with a CRC).
    pub header: usize,
    /// The whole frame's length, header included.
    pub length: usize,
    /// More than one raw data block in the frame.
    pub blocks: u8,
}

impl Adts {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 7 || data[0] != 0xFF || data[1] & 0xF6 != 0xF0 {
            return None;
        }
        let rate_index = data[2] >> 2 & 0x0F;
        let length = (usize::from(data[3] & 0x03) << 11)
            | (usize::from(data[4]) << 3)
            | usize::from(data[5] >> 5);
        let header = if data[1] & 1 == 1 { 7 } else { 9 };
        if usize::from(rate_index) >= AAC_RATES.len() || length < header {
            return None;
        }
        Some(Self {
            object_type: (data[2] >> 6) + 1,
            rate_index,
            channels: (data[2] & 1) << 2 | data[3] >> 6,
            header,
            length,
            blocks: (data[6] & 0x03) + 1,
        })
    }

    pub fn rate(&self) -> u32 {
        AAC_RATES[usize::from(self.rate_index)]
    }

    /// The two-byte AudioSpecificConfig.
    pub fn config(&self) -> Vec<u8> {
        let value = u16::from(self.object_type) << 11
            | u16::from(self.rate_index) << 7
            | u16::from(self.channels) << 3;
        value.to_be_bytes().to_vec()
    }
}

/// What an AudioSpecificConfig says: object type, sampling rate and channels.
pub fn parse_audio_config(config: &[u8]) -> Option<(u8, u32, u8)> {
    let mut bits = Bits::new(config);
    let mut object_type = bits.bits(5)? as u8;
    if object_type == 31 {
        object_type = 32 + bits.bits(6)? as u8;
    }
    let index = bits.bits(4)?;
    let rate = if index == 15 {
        bits.bits(24)?
    } else {
        *AAC_RATES.get(index as usize)?
    };
    Some((object_type, rate, bits.bits(4)? as u8))
}

/// An AudioSpecificConfig for AAC LC at `rate` with `channels`.
pub fn aac_config(rate: u32, channels: u8) -> Vec<u8> {
    let index = AAC_RATES
        .iter()
        .position(|known| *known == rate)
        .unwrap_or(4) as u16;
    (2u16 << 11 | index << 7 | u16::from(channels.min(7)) << 3)
        .to_be_bytes()
        .to_vec()
}

/// One MPEG audio frame's header (layers I to III).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MpegAudio {
    pub layer: u8,
    /// MPEG-1 (true) or MPEG-2/2.5.
    pub mpeg1: bool,
    pub rate: u32,
    pub channels: u8,
    pub length: usize,
    pub samples: u32,
}

impl MpegAudio {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 || data[0] != 0xFF || data[1] & 0xE0 != 0xE0 {
            return None;
        }
        let version = data[1] >> 3 & 3;
        let layer = match data[1] >> 1 & 3 {
            3 => 1,
            2 => 2,
            1 => 3,
            _ => return None,
        };
        let bitrate_index = usize::from(data[2] >> 4);
        let rate_index = usize::from(data[2] >> 2 & 3);
        if version == 1 || bitrate_index == 0 || bitrate_index == 15 || rate_index == 3 {
            return None;
        }
        let mpeg1 = version == 3;
        const V1: [[u32; 15]; 3] = [
            [
                0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
            ],
            [
                0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
            ],
            [
                0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
            ],
        ];
        const V2: [[u32; 15]; 2] = [
            [
                0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
            ],
            [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
        ];
        let kbps = if mpeg1 {
            V1[usize::from(layer - 1)][bitrate_index]
        } else {
            V2[usize::from(layer.min(2) - 1)][bitrate_index]
        };
        let rate = match version {
            3 => [44_100, 48_000, 32_000][rate_index],
            2 => [22_050, 24_000, 16_000][rate_index],
            _ => [11_025, 12_000, 8_000][rate_index],
        };
        let padding = u32::from(data[2] >> 1 & 1);
        let bits = kbps * 1000;
        let (length, samples) = match layer {
            1 => ((12 * bits / rate + padding) * 4, 384),
            2 => (144 * bits / rate + padding, 1152),
            _ if mpeg1 => (144 * bits / rate + padding, 1152),
            _ => (72 * bits / rate + padding, 576),
        };
        Some(Self {
            layer,
            mpeg1,
            rate,
            channels: if data[3] >> 6 == 3 { 1 } else { 2 },
            length: length as usize,
            samples,
        })
    }
}

/// One AC-3 or E-AC-3 sync frame's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ac3 {
    pub enhanced: bool,
    pub rate: u32,
    pub channels: u8,
    pub length: usize,
    pub samples: u32,
    /// For `dac3`.
    pub fscod: u8,
    pub bsid: u8,
    pub bsmod: u8,
    pub acmod: u8,
    pub lfe: bool,
    pub bit_rate_code: u8,
}

/// AC-3 frame sizes in 16-bit words by `frmsizecod`, at 48, 44.1 and 32 kHz.
const AC3_WORDS: [[u16; 3]; 38] = [
    [64, 69, 96],
    [64, 70, 96],
    [80, 87, 120],
    [80, 88, 120],
    [96, 104, 144],
    [96, 105, 144],
    [112, 121, 168],
    [112, 122, 168],
    [128, 139, 192],
    [128, 140, 192],
    [160, 174, 240],
    [160, 175, 240],
    [192, 208, 288],
    [192, 209, 288],
    [224, 243, 336],
    [224, 244, 336],
    [256, 278, 384],
    [256, 279, 384],
    [320, 348, 480],
    [320, 349, 480],
    [384, 417, 576],
    [384, 418, 576],
    [448, 487, 672],
    [448, 488, 672],
    [512, 557, 768],
    [512, 558, 768],
    [640, 696, 960],
    [640, 697, 960],
    [768, 835, 1152],
    [768, 836, 1152],
    [896, 975, 1344],
    [896, 976, 1344],
    [1024, 1114, 1536],
    [1024, 1115, 1536],
    [1152, 1253, 1728],
    [1152, 1254, 1728],
    [1280, 1393, 1920],
    [1280, 1394, 1920],
];

const AC3_CHANNELS: [u8; 8] = [2, 1, 2, 3, 3, 4, 4, 5];

impl Ac3 {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 || data[0] != 0x0B || data[1] != 0x77 {
            return None;
        }
        let bsid = data[5] >> 3;
        if bsid > 10 {
            // E-AC-3.
            let mut bits = Bits::new(&data[2..]);
            bits.bits(2)?;
            bits.bits(3)?;
            let length = (bits.bits(11)? as usize + 1) * 2;
            let fscod = bits.bits(2)? as u8;
            let (rate, blocks) = if fscod == 3 {
                let reduced = bits.bits(2)?;
                ([24_000, 22_050, 16_000].get(reduced as usize).copied()?, 6)
            } else {
                let blocks = [1, 2, 3, 6][bits.bits(2)? as usize];
                ([48_000, 44_100, 32_000][usize::from(fscod)], blocks)
            };
            let acmod = bits.bits(3)? as u8;
            let lfe = bits.bit()?;
            return Some(Self {
                enhanced: true,
                rate,
                channels: AC3_CHANNELS[usize::from(acmod)] + u8::from(lfe),
                length,
                samples: 256 * blocks,
                fscod,
                bsid,
                bsmod: 0,
                acmod,
                lfe,
                bit_rate_code: 0,
            });
        }
        let fscod = data[4] >> 6;
        let frmsizecod = data[4] & 0x3F;
        let words = AC3_WORDS.get(usize::from(frmsizecod))?;
        let length = usize::from(*words.get(usize::from(fscod))?) * 2;
        let rate = [48_000, 44_100, 32_000][usize::from(fscod)];
        let bsmod = data[5] & 7;
        let mut bits = Bits::new(&data[6..]);
        let acmod = bits.bits(3)? as u8;
        if acmod & 1 == 1 && acmod != 1 {
            bits.bits(2)?;
        }
        if acmod & 4 != 0 {
            bits.bits(2)?;
        }
        if acmod == 2 {
            bits.bits(2)?;
        }
        let lfe = bits.bit()?;
        Some(Self {
            enhanced: false,
            rate,
            channels: AC3_CHANNELS[usize::from(acmod)] + u8::from(lfe),
            length,
            samples: 1536,
            fscod,
            bsid,
            bsmod,
            acmod,
            lfe,
            bit_rate_code: frmsizecod >> 1,
        })
    }
}

/// How many 48 kHz samples an Opus packet holds, from its TOC byte.
pub fn opus_samples(packet: &[u8]) -> u32 {
    let Some(&toc) = packet.first() else {
        return 960;
    };
    let config = toc >> 3;
    let frame = match config {
        0..=11 => [480, 960, 1920, 2880][usize::from(config % 4)],
        12..=15 => [480, 960][usize::from(config % 2)],
        _ => [120, 240, 480, 960][usize::from(config % 4)],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => packet.get(1).map_or(1, |count| u32::from(count & 0x3F)),
    };
    frame * frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annex_b_splits_at_three_and_four_byte_start_codes() {
        let data = [
            0, 0, 0, 1, 0x09, 0xF0, 0, 0, 1, 0x67, 1, 2, 0, 0, 0, 1, 0x65, 9,
        ];
        let units = nal_units(&data);
        assert_eq!(units, vec![&[0x09, 0xF0][..], &[0x67, 1, 2], &[0x65, 9]]);
        assert_eq!(
            length_prefixed(units.into_iter().skip(1)),
            vec![0, 0, 0, 3, 0x67, 1, 2, 0, 0, 0, 2, 0x65, 9]
        );
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3]), vec![0, 0, 1, 0, 0]);
    }

    #[test]
    fn adts_headers_become_audio_configs() {
        // AAC LC, 44.1 kHz, stereo, 371 bytes, no CRC.
        let header = [0xFF, 0xF1, 0x50, 0x80, 0x2E, 0x7F, 0xFC];
        let adts = Adts::parse(&header).unwrap();
        assert_eq!(
            (adts.object_type, adts.rate(), adts.channels, adts.length),
            (2, 44_100, 2, 371)
        );
        assert_eq!(adts.config(), vec![0x12, 0x10]);
        assert_eq!(parse_audio_config(&[0x12, 0x10]), Some((2, 44_100, 2)));
        assert_eq!(aac_config(48_000, 2), vec![0x11, 0x90]);
    }

    #[test]
    fn mpeg_audio_and_ac3_frames_have_their_lengths() {
        // MPEG-1 layer III, 128 kbps, 44.1 kHz, no padding: 417 bytes.
        let mp3 = MpegAudio::parse(&[0xFF, 0xFB, 0x90, 0x64]).unwrap();
        assert_eq!(
            (mp3.layer, mp3.rate, mp3.length, mp3.samples),
            (3, 44_100, 417, 1152)
        );
        // AC-3 at 48 kHz, 384 kbps (frmsizecod 28), 3/2 with LFE.
        let ac3 = Ac3::parse(&[0x0B, 0x77, 0, 0, 0x1C, 0x40, 0xE1, 0]).unwrap();
        assert_eq!((ac3.rate, ac3.length, ac3.channels), (48_000, 1536, 6));
        assert_eq!(ac3.bit_rate_code, 14);
        assert_eq!(opus_samples(&[0xFC]), 960);
        assert_eq!(opus_samples(&[0x7B, 0x83]), 2880);
    }
}
