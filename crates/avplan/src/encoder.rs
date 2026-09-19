//! Choosing an encoder from the ones a machine actually has.

use std::sync::LazyLock;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A line of an encoder listing: flags, then the name.
static LISTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*[A-Z.]{6}\s+(\S+)").expect("a valid pattern"));

/// An H.264 encoder, in the order they are preferred.
///
/// Hardware first, because a re-encode of a two-hour recording is the
/// difference between three minutes and forty. The software encoder is last
/// and is always there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Encoder {
    /// NVIDIA's hardware encoder.
    H264Nvenc,
    /// Apple's, on Apple silicon and recent Macs.
    H264Videotoolbox,
    /// The one AMD and Intel graphics offer through VAAPI.
    H264Vaapi,
    /// Software. Always available, and the slowest by a wide margin.
    Libx264,
    /// The stream is already H.264 or HEVC and is copied rather than encoded.
    Copy,
}

impl Encoder {
    /// What ffmpeg calls it.
    pub fn name(self) -> &'static str {
        match self {
            Encoder::H264Nvenc => "h264_nvenc",
            Encoder::H264Videotoolbox => "h264_videotoolbox",
            Encoder::H264Vaapi => "h264_vaapi",
            Encoder::Libx264 => "libx264",
            Encoder::Copy => "copy",
        }
    }

    /// Whether this one runs on the graphics card.
    pub fn is_hardware(self) -> bool {
        matches!(
            self,
            Encoder::H264Nvenc | Encoder::H264Videotoolbox | Encoder::H264Vaapi
        )
    }

    /// The arguments that have to come before the input.
    ///
    /// Only VAAPI needs any: the device has to be opened before the file is,
    /// so a filter can upload frames to it.
    pub fn before_input(self, vaapi_device: &str) -> Vec<String> {
        match self {
            Encoder::H264Vaapi => vec![
                "-init_hw_device".into(),
                format!("vaapi=va:{vaapi_device}"),
                "-filter_hw_device".into(),
                "va".into(),
            ],
            _ => Vec::new(),
        }
    }

    /// The arguments that set up the video stream.
    ///
    /// Each encoder is asked for roughly the same quality, in whichever of
    /// the four quality dialects it happens to speak.
    pub fn video_arguments(self) -> Vec<String> {
        let args: &[&str] = match self {
            Encoder::Copy => &["-c:v", "copy"],
            Encoder::H264Nvenc => &[
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p5",
                "-rc",
                "vbr",
                "-cq",
                "23",
                "-b:v",
                "0",
            ],
            Encoder::H264Videotoolbox => &[
                "-c:v",
                "h264_videotoolbox",
                "-q:v",
                "60",
                "-profile:v",
                "high",
                "-pix_fmt",
                "yuv420p",
            ],
            // Software decode and an upload to the card, so that a source the
            // GPU's own decoder does not understand still encodes.
            Encoder::H264Vaapi => &[
                "-vf",
                "format=nv12,hwupload",
                "-c:v",
                "h264_vaapi",
                "-qp",
                "23",
            ],
            Encoder::Libx264 => &[
                "-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p",
            ],
        };
        args.iter().map(|arg| (*arg).to_string()).collect()
    }
}

/// The encoders this knows how to use, best first.
pub const PREFERRED: &[Encoder] = &[
    Encoder::H264Nvenc,
    Encoder::H264Videotoolbox,
    Encoder::H264Vaapi,
    Encoder::Libx264,
];

/// The best encoder present in an encoder listing.
///
/// `None` when the listing has none of them, which means the installed
/// ffmpeg cannot produce H.264 at all.
pub fn best_available(listing: &str) -> Option<Encoder> {
    let available: Vec<&str> = LISTED
        .captures_iter(listing)
        .map(|found| found.get(1).expect("a name").as_str())
        .collect();

    PREFERRED
        .iter()
        .copied()
        .find(|encoder| available.contains(&encoder.name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
Encoders:
 V..... = Video
 ------
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC
 V....D libx265              libx265 HEVC
 V..... h264_videotoolbox    VideoToolbox H.264 Encoder
 A..... aac                  AAC (Advanced Audio Coding)
";

    #[test]
    fn the_best_one_present_is_chosen() {
        assert_eq!(best_available(LISTING), Some(Encoder::H264Videotoolbox));
    }

    #[test]
    fn hardware_beats_software() {
        let with_nvenc = format!("{LISTING} V..... h264_nvenc           NVIDIA NVENC H.264\n");
        assert_eq!(best_available(&with_nvenc), Some(Encoder::H264Nvenc));
    }

    #[test]
    fn software_is_the_last_resort_and_not_the_first() {
        let software_only = " V....D libx264              libx264 H.264\n";
        assert_eq!(best_available(software_only), Some(Encoder::Libx264));
    }

    #[test]
    fn an_ffmpeg_that_cannot_make_h264_says_so() {
        assert_eq!(
            best_available(" V....D libx265              libx265 HEVC\n"),
            None
        );
        assert_eq!(best_available(""), None);
    }

    #[test]
    fn a_name_that_merely_contains_an_encoder_name_is_not_one() {
        // `libx264rgb` is a different encoder, and matching loosely would
        // pick it for a plan that then produces the wrong pixel format.
        let other = " V....D libx264rgb           libx264 H.264 RGB\n";
        assert_eq!(best_available(other), None);
    }

    #[test]
    fn only_vaapi_needs_the_device_opened_first() {
        assert_eq!(
            Encoder::H264Vaapi.before_input("/dev/dri/renderD128"),
            [
                "-init_hw_device",
                "vaapi=va:/dev/dri/renderD128",
                "-filter_hw_device",
                "va"
            ]
        );
        assert!(
            Encoder::H264Nvenc
                .before_input("/dev/dri/renderD128")
                .is_empty()
        );
        assert!(Encoder::Libx264.before_input("x").is_empty());
    }

    #[test]
    fn every_encoder_names_itself_in_its_own_arguments() {
        for encoder in PREFERRED {
            let args = encoder.video_arguments();
            assert!(
                args.contains(&encoder.name().to_string()),
                "{:?} does not name itself",
                encoder
            );
        }
    }

    #[test]
    fn copying_is_not_encoding() {
        assert!(!Encoder::Copy.is_hardware());
        assert_eq!(Encoder::Copy.video_arguments(), ["-c:v", "copy"]);
    }
}
