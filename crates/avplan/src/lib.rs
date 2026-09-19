//! Whether a recording plays as it is, and the cheapest way to fix it if not.
//!
//! A player on a phone or a tablet decodes a short list of things. A file
//! outside that list does not fail loudly — it plays with no picture, or no
//! sound, or refuses to open with an error that names nothing. Converting
//! everything to be safe costs hours of encoding for files that were already
//! fine.
//!
//! So: read what the streams actually are, and do the least that fixes them.
//! An already-H.264 picture is copied rather than re-encoded, which turns
//! forty minutes of work into forty seconds of copying. Nothing here runs
//! ffmpeg; it prints the call.

pub mod encoder;

use std::sync::LazyLock;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use encoder::Encoder;

/// Video a player decodes natively.
static NATIVE_VIDEO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(avc1|h264|hvc1|hev1|hevc)").expect("a valid pattern"));

/// Audio a player decodes natively.
///
/// Opus inside an MP4 gives a video with no sound at all, which looks like a
/// broken file rather than a codec problem.
static NATIVE_AUDIO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(aac|mp4a)").expect("a valid pattern"));

/// Containers a player can open.
///
/// Matroska is not among them whatever is inside it, which is why the
/// container is checked separately from the streams.
const PLAYABLE_CONTAINERS: &[&str] = &["m4v", "mov", "mp4"];

/// The audio settings a re-encode uses: stereo, and enough bitrate that
/// speech is indistinguishable from the original.
const AUDIO_ARGUMENTS: &[&str] = &["-c:a", "aac", "-b:a", "160k", "-ac", "2"];

/// The render node a machine's first graphics card usually appears as.
pub const DEFAULT_VAAPI_DEVICE: &str = "/dev/dri/renderD128";

/// What a file's streams are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Streams {
    /// The video codec, as the prober names it.
    pub video: Option<String>,
    /// The audio codec.
    pub audio: Option<String>,
    /// The file's extension, lowercased, without the dot.
    pub container: Option<String>,
}

/// What has to happen to a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Plan {
    /// Record version.
    #[serde(default = "version")]
    pub v: u32,
    /// The file the plan is about.
    pub path: String,
    /// Whether it plays as it stands.
    pub playable: bool,
    /// What its streams are.
    pub streams: Streams,
    /// Why it does not play, in the order the reasons were found. Empty when
    /// it does.
    pub reasons: Vec<String>,
    /// Which encoder the picture would go through, or `copy`.
    pub encoder: Option<Encoder>,
    /// Whether the sound is copied rather than re-encoded.
    pub copies_audio: bool,
    /// The call that would fix it, argument by argument. Empty when nothing
    /// needs doing.
    pub argv: Vec<String>,
}

fn version() -> u32 {
    corpus_core::V
}

impl corpus_core::Record for Plan {
    fn version(&self) -> u32 {
        self.v
    }
}

/// Whether a video stream plays as it is.
pub fn video_plays(codec: Option<&str>) -> bool {
    codec.is_some_and(|codec| NATIVE_VIDEO.is_match(codec))
}

/// Whether an audio stream plays as it is.
pub fn audio_plays(codec: Option<&str>) -> bool {
    codec.is_some_and(|codec| NATIVE_AUDIO.is_match(codec))
}

/// Whether a container can be opened at all.
pub fn container_plays(extension: Option<&str>) -> bool {
    extension.is_some_and(|extension| {
        PLAYABLE_CONTAINERS
            .binary_search(&extension.to_lowercase().as_str())
            .is_ok()
    })
}

/// The extension of a path, lowercased, without the dot.
///
/// Read off the last segment of the path as written, rather than by asking
/// the path library. The library folds `.` and `..` away and then reports the
/// extension of whatever they were attached to, which makes `film.mp4/.` —
/// a directory — a playable MP4.
pub fn container_of(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches(std::path::is_separator);
    let segment = trimmed.rsplit(std::path::is_separator).next().unwrap_or("");
    if matches!(segment, "" | "." | "..") {
        return None;
    }
    // A leading dot is a hidden file, not an extension: `.mp4` is a name.
    match segment.rfind('.') {
        None | Some(0) => None,
        Some(at) => match &segment[at + 1..] {
            "" => None,
            extension => Some(extension.to_lowercase()),
        },
    }
}

/// Where a converted copy goes: beside the original, named for what it is.
pub fn output_beside(path: &str) -> String {
    let path = std::path::Path::new(path);
    let stem = path.file_stem().map(|stem| stem.to_string_lossy());
    match (path.parent(), stem) {
        (Some(parent), Some(stem)) if !parent.as_os_str().is_empty() => {
            // The root is the one directory that already ends in a separator,
            // and a name beginning with two of them means something else
            // again on some systems.
            let parent = parent.display().to_string();
            match parent.ends_with(std::path::MAIN_SEPARATOR) {
                true => format!("{parent}{stem}.h264.mp4"),
                false => format!("{parent}/{stem}.h264.mp4"),
            }
        }
        (_, Some(stem)) => format!("{stem}.h264.mp4"),
        _ => format!("{}.h264.mp4", path.display()),
    }
}

/// A path an encoder will read as a path.
///
/// The destination is the one argument with no flag in front of it, so a name
/// beginning with a dash would be read as an option and the call would fail
/// on a file that is otherwise fine. `./` in front of it names the same file
/// and cannot be mistaken for anything.
fn runnable(path: &str) -> String {
    match path.starts_with('-') {
        true => format!("./{path}"),
        false => path.to_string(),
    }
}

/// Judge a file and, if it needs converting, say how.
///
/// `available` is the best encoder the machine has; it is only consulted when
/// the picture actually has to be re-encoded.
pub fn plan(
    path: &str,
    streams: &Streams,
    available: Option<Encoder>,
    output: &str,
    vaapi_device: &str,
) -> Plan {
    let video_ok = video_plays(streams.video.as_deref());
    let audio_ok = audio_plays(streams.audio.as_deref());
    let container_ok = container_plays(streams.container.as_deref());

    let mut reasons = Vec::new();
    if !video_ok {
        reasons.push(match streams.video.as_deref() {
            None | Some("") => "no video stream this can read".to_string(),
            Some(codec) => format!("the picture is {codec}"),
        });
    }
    if !audio_ok {
        reasons.push(match streams.audio.as_deref() {
            None | Some("") => "no audio stream this can read".to_string(),
            Some(codec) => format!("the sound is {codec}"),
        });
    }
    if !container_ok {
        reasons.push(match streams.container.as_deref() {
            None => "the file has no extension to go on".to_string(),
            Some(extension) => format!("a .{extension} container cannot be opened"),
        });
    }

    if reasons.is_empty() {
        return Plan {
            v: corpus_core::V,
            path: path.to_string(),
            playable: true,
            streams: streams.clone(),
            reasons,
            encoder: None,
            copies_audio: true,
            argv: Vec::new(),
        };
    }

    // The picture is the expensive part, and it only has to be re-encoded
    // when it is not already something a player decodes. A file whose only
    // problem is its container or its sound is a copy and a remux.
    let encoder = match video_ok {
        true => Some(Encoder::Copy),
        false => available,
    };

    let argv = match encoder {
        None => Vec::new(),
        Some(encoder) => {
            let mut argv = vec!["-nostdin".to_string(), "-y".to_string()];
            argv.extend(encoder.before_input(vaapi_device));
            argv.push("-i".into());
            argv.push(path.to_string());
            argv.extend(encoder.video_arguments());
            match audio_ok {
                true => argv.extend(["-c:a".to_string(), "copy".to_string()]),
                false => argv.extend(AUDIO_ARGUMENTS.iter().map(|arg| (*arg).to_string())),
            }
            // So a player can start before the whole file has arrived.
            argv.extend(["-movflags".to_string(), "+faststart".to_string()]);
            argv.push(runnable(output));
            argv
        }
    };

    Plan {
        v: corpus_core::V,
        path: path.to_string(),
        playable: false,
        streams: streams.clone(),
        reasons,
        encoder,
        copies_audio: audio_ok,
        argv,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_that_looks_like_a_file_is_not_one() {
        // `Path` folds a trailing `.` away and would otherwise read the
        // extension of the directory it was attached to, which makes a
        // directory a playable MP4.
        assert_eq!(container_of("film.mp4/."), None);
        assert_eq!(container_of("film.mp4/./"), None);
        assert_eq!(container_of("film.mp4/.."), None);
        assert_eq!(container_of("film.mp4"), Some("mp4".to_string()));
        assert_eq!(container_of("film.MP4"), Some("mp4".to_string()));
        assert_eq!(container_of("film.mp4//"), Some("mp4".to_string()));
        assert_eq!(container_of("film"), None);
        // A leading dot is a hidden file rather than an extension.
        assert_eq!(container_of(".mp4"), None);
        assert_eq!(container_of("/media/.hidden.mp4"), Some("mp4".to_string()));
        assert_eq!(container_of("talk."), None);
        assert_eq!(container_of(""), None);
        assert_eq!(container_of("/"), None);
    }

    #[test]
    fn a_destination_beginning_with_a_dash_is_named_so_it_is_read_as_one() {
        let found = plan(
            "-clip.webm",
            &streams("vp9", "opus", "webm"),
            Some(Encoder::Libx264),
            "-clip.h264.mp4",
            DEFAULT_VAAPI_DEVICE,
        );
        assert_eq!(
            found.argv.last().map(String::as_str),
            Some("./-clip.h264.mp4")
        );
        // The source needs no such help: it comes after `-i`.
        assert!(found.argv.contains(&"-clip.webm".to_string()));
    }

    #[test]
    fn a_file_at_the_root_does_not_get_two_leading_separators() {
        // A name beginning with exactly two separators is left to the system
        // to interpret, and some interpret it as something other than a path.
        assert_eq!(output_beside("/talk.webm"), "/talk.h264.mp4");
        assert_eq!(output_beside("/media/talk.webm"), "/media/talk.h264.mp4");
        assert_eq!(output_beside("talk.webm"), "talk.h264.mp4");
    }

    fn streams(video: &str, audio: &str, container: &str) -> Streams {
        Streams {
            video: Some(video.into()),
            audio: Some(audio.into()),
            container: Some(container.into()),
        }
    }

    fn plan_for(path: &str, streams: &Streams) -> Plan {
        plan(
            path,
            streams,
            Some(Encoder::H264Nvenc),
            &output_beside(path),
            DEFAULT_VAAPI_DEVICE,
        )
    }

    #[test]
    fn the_container_list_is_sorted_so_it_can_be_searched() {
        assert!(PLAYABLE_CONTAINERS.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_file_that_already_plays_needs_nothing() {
        let found = plan_for("talk.mp4", &streams("h264", "aac", "mp4"));
        assert!(found.playable);
        assert!(found.argv.is_empty());
        assert!(found.reasons.is_empty());
        assert_eq!(found.encoder, None);
    }

    #[test]
    fn the_codec_spellings_a_prober_uses_are_all_recognised() {
        for video in [
            "h264",
            "avc1",
            "avc1.640028",
            "hevc",
            "hvc1",
            "hev1",
            "H264",
        ] {
            assert!(video_plays(Some(video)), "{video}");
        }
        for audio in ["aac", "mp4a", "mp4a.40.2", "AAC"] {
            assert!(audio_plays(Some(audio)), "{audio}");
        }
        assert!(!video_plays(Some("vp9")));
        assert!(!audio_plays(Some("opus")));
        assert!(!video_plays(None));
    }

    #[test]
    fn a_picture_that_already_plays_is_copied_rather_than_re_encoded() {
        let found = plan_for("talk.mkv", &streams("h264", "aac", "mkv"));
        assert!(!found.playable);
        assert_eq!(found.encoder, Some(Encoder::Copy));
        assert!(found.copies_audio);
        assert!(
            found.argv.windows(2).any(|pair| pair == ["-c:v", "copy"]),
            "{:?}",
            found.argv
        );
        assert!(found.argv.windows(2).any(|pair| pair == ["-c:a", "copy"]));
    }

    #[test]
    fn only_the_sound_being_wrong_costs_only_the_sound() {
        let found = plan_for("talk.mp4", &streams("h264", "opus", "mp4"));
        assert_eq!(found.encoder, Some(Encoder::Copy));
        assert!(!found.copies_audio);
        assert!(found.argv.windows(2).any(|pair| pair == ["-c:v", "copy"]));
        assert!(found.argv.windows(2).any(|pair| pair == ["-c:a", "aac"]));
        assert_eq!(found.reasons, ["the sound is opus"]);
    }

    #[test]
    fn a_picture_that_does_not_play_is_re_encoded() {
        let found = plan_for("talk.webm", &streams("vp9", "opus", "webm"));
        assert_eq!(found.encoder, Some(Encoder::H264Nvenc));
        assert!(found.argv.contains(&"h264_nvenc".to_string()));
        assert_eq!(
            found.reasons,
            [
                "the picture is vp9",
                "the sound is opus",
                "a .webm container cannot be opened"
            ]
        );
    }

    #[test]
    fn the_call_is_in_the_order_ffmpeg_wants_it() {
        let found = plan(
            "talk.webm",
            &streams("av1", "opus", "webm"),
            Some(Encoder::H264Vaapi),
            "out.mp4",
            "/dev/dri/renderD129",
        );
        let at = |arg: &str| found.argv.iter().position(|a| a == arg).expect(arg);
        assert!(at("-init_hw_device") < at("-i"), "the device opens first");
        assert!(at("-i") < at("-c:v"), "the input comes before the encoder");
        assert!(at("-c:v") < at("-movflags"));
        assert_eq!(found.argv.last().map(String::as_str), Some("out.mp4"));
        assert!(
            found
                .argv
                .contains(&"vaapi=va:/dev/dri/renderD129".to_string())
        );
    }

    #[test]
    fn a_machine_with_no_h264_encoder_gets_no_call() {
        let found = plan(
            "talk.webm",
            &streams("vp9", "opus", "webm"),
            None,
            "out.mp4",
            DEFAULT_VAAPI_DEVICE,
        );
        assert!(!found.playable);
        assert_eq!(found.encoder, None);
        assert!(found.argv.is_empty(), "nothing on this machine could do it");
    }

    #[test]
    fn a_missing_stream_is_a_reason_of_its_own() {
        let found = plan_for(
            "sound.mp4",
            &Streams {
                video: None,
                audio: Some("aac".into()),
                container: Some("mp4".into()),
            },
        );
        assert_eq!(found.reasons, ["no video stream this can read"]);
    }

    #[test]
    fn the_converted_copy_goes_beside_the_original() {
        assert_eq!(output_beside("talk.webm"), "talk.h264.mp4");
        assert_eq!(output_beside("a/b/talk.webm"), "a/b/talk.h264.mp4");
        assert_eq!(output_beside("talk"), "talk.h264.mp4");
    }

    #[test]
    fn a_container_is_read_from_the_name() {
        assert_eq!(container_of("a/b/talk.WEBM").as_deref(), Some("webm"));
        assert_eq!(container_of("talk").as_deref(), None);
        assert!(container_plays(Some("MP4")));
        assert!(!container_plays(Some("mkv")));
        assert!(!container_plays(None));
    }
}
