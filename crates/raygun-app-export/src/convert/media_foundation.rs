//! Whether Windows' own media stack opens a file: Media Foundation, which Media Player and Films &
//! TV play through. A file passes when its source reader opens it, says how long it is, and
//! decodes frames of its first picture and sound streams.

use std::path::Path;

use windows::Win32::Media::MediaFoundation::{
    IMFSourceReader, MF_E_INVALIDSTREAMNUMBER, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_PD_DURATION,
    MF_SOURCE_READER_ANY_STREAM, MF_SOURCE_READER_MEDIASOURCE, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READERF_ERROR, MF_VERSION, MFAudioFormat_PCM, MFCreateMediaType,
    MFCreateSourceReaderFromURL, MFMediaType_Audio, MFMediaType_Video, MFSTARTUP_NOSOCKET,
    MFShutdown, MFStartup, MFVideoFormat_NV12,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::{GUID, HSTRING};

/// What Media Foundation found in a file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opened {
    pub seconds: f64,
    /// A picture stream decoded a frame.
    pub video: bool,
    /// A sound stream decoded a frame.
    pub audio: bool,
}

/// Samples read at most while looking for a decoded frame of each stream.
const READS: usize = 2000;

/// Opens `path` with Media Foundation and decodes the start of its first picture and sound
/// streams. Runs on a thread of its own (COM is set up there and nowhere else).
pub fn open(path: &Path) -> Result<Opened, String> {
    let path = path.to_owned();
    std::thread::spawn(move || {
        // SAFETY: COM and Media Foundation are started and stopped on this thread only, and
        // every interface is released before they stop.
        unsafe { open_here(&path) }
    })
    .join()
    .map_err(|_| "Media Foundation stopped while reading the file".to_owned())?
}

/// Media Foundation and COM, stopped when dropped.
struct Started {
    com: bool,
}

impl Drop for Started {
    fn drop(&mut self) {
        // SAFETY: started on this thread by `open_here`.
        unsafe {
            let _ = MFShutdown();
            if self.com {
                CoUninitialize();
            }
        }
    }
}

/// A stream asked for decoded: its index and whether it's the picture.
struct Wanted {
    index: u32,
    video: bool,
    decoded: bool,
    ended: bool,
}

unsafe fn open_here(path: &Path) -> Result<Opened, String> {
    let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET) }.map_err(|error| {
        if com {
            unsafe { CoUninitialize() };
        }
        format!("Media Foundation didn't start ({error})")
    })?;
    let _started = Started { com };
    let url = HSTRING::from(path.as_os_str());
    let reader = unsafe { MFCreateSourceReaderFromURL(&url, None) }
        .map_err(|error| format!("Windows' players can't open it ({})", error.message()))?;
    let duration = unsafe {
        reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
    }
    .map_err(|error| format!("Windows' players don't know how long it is ({error})"))?;
    // SAFETY: MF_PD_DURATION is a VT_UI8 value (100-nanosecond units).
    let ticks = unsafe { duration.Anonymous.Anonymous.Anonymous.uhVal };
    let wanted = unsafe { choose_streams(&reader)? };
    let mut wanted = wanted;
    unsafe { read_frames(&reader, &mut wanted)? };
    Ok(Opened {
        seconds: ticks as f64 / 10_000_000.0,
        video: wanted.iter().any(|stream| stream.video && stream.decoded),
        audio: wanted.iter().any(|stream| !stream.video && stream.decoded),
    })
}

/// Selects the first picture and sound streams, each asked for decoded (which loads its decoder:
/// NV12 pictures, PCM sound); every other stream is left out.
unsafe fn choose_streams(reader: &IMFSourceReader) -> Result<Vec<Wanted>, String> {
    let mut wanted: Vec<Wanted> = Vec::new();
    let mut index = 0u32;
    loop {
        let native = match unsafe { reader.GetNativeMediaType(index, 0) } {
            Ok(native) => native,
            Err(error) if error.code() == MF_E_INVALIDSTREAMNUMBER => break,
            Err(error) => return Err(format!("Windows' players can't read a stream ({error})")),
        };
        let major = unsafe { native.GetGUID(&MF_MT_MAJOR_TYPE) }
            .map_err(|error| format!("A stream has no type ({error})"))?;
        let video = major == MFMediaType_Video;
        let first = (video || major == MFMediaType_Audio)
            && !wanted.iter().any(|stream| stream.video == video);
        unsafe { reader.SetStreamSelection(index, first) }.map_err(|error| error.to_string())?;
        if first {
            let (kind, subtype) = if video {
                ("picture", MFVideoFormat_NV12)
            } else {
                ("sound", MFAudioFormat_PCM)
            };
            unsafe { decoded_as(reader, index, major, subtype) }
                .map_err(|error| format!("Windows has no decoder for its {kind} ({error})"))?;
            wanted.push(Wanted {
                index,
                video,
                decoded: false,
                ended: false,
            });
        }
        index += 1;
    }
    Ok(wanted)
}

unsafe fn decoded_as(
    reader: &IMFSourceReader,
    index: u32,
    major: GUID,
    subtype: GUID,
) -> windows::core::Result<()> {
    unsafe {
        let wanted = MFCreateMediaType()?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &major)?;
        wanted.SetGUID(&MF_MT_SUBTYPE, &subtype)?;
        reader.SetCurrentMediaType(index, None, &wanted)
    }
}

/// Reads the selected streams in turn until each has given a decoded frame (or ended).
unsafe fn read_frames(reader: &IMFSourceReader, wanted: &mut [Wanted]) -> Result<(), String> {
    for _ in 0..READS {
        if wanted.iter().all(|stream| stream.decoded || stream.ended) {
            break;
        }
        let mut flags = 0u32;
        let mut actual = 0u32;
        let mut stamp = 0i64;
        let mut sample = None;
        unsafe {
            reader.ReadSample(
                MF_SOURCE_READER_ANY_STREAM.0 as u32,
                0,
                Some(&mut actual),
                Some(&mut flags),
                Some(&mut stamp),
                Some(&mut sample),
            )
        }
        .map_err(|error| format!("It didn't decode ({error})"))?;
        if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
            return Err("Windows' players stopped reading it".into());
        }
        let Some(stream) = wanted.iter_mut().find(|stream| stream.index == actual) else {
            continue;
        };
        if sample.is_some() {
            stream.decoded = true;
        }
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            stream.ended = true;
        }
    }
    match wanted.iter().find(|stream| !stream.decoded) {
        Some(stream) if stream.video => Err("Its picture decoded nothing".into()),
        Some(_) => Err("Its sound decoded nothing".into()),
        None => Ok(()),
    }
}
