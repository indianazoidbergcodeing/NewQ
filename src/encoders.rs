//! The video encoders NewQ knows by FFmpeg's name, and what is known about
//! them: which codec each makes, whether it uses the graphics hardware or the
//! processor, how to call it, and which protocols can carry the codec.
//!
//! What a given FFmpeg has, and whether an encoder works on this computer, is
//! found out in `stream` (`encoder_present`, `test_encoder`); this is only the
//! list. The arguments each encoder takes are in `stream::video_args`.

use crate::show::{ServerProtocol, VideoCodec};

/// Whether an encoder uses the graphics hardware or the processor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Hardware,
    Software,
}

#[derive(Clone, Copy, Debug)]
pub struct Encoder {
    /// FFmpeg's name for it.
    pub name: &'static str,
    pub codec: VideoCodec,
    pub kind: Kind,
    /// What it is, as a person would say it.
    pub label: &'static str,
}

const fn enc(name: &'static str, codec: VideoCodec, kind: Kind, label: &'static str) -> Encoder {
    Encoder {
        name,
        codec,
        kind,
        label,
    }
}

use Kind::{Hardware as Hw, Software as Sw};
use VideoCodec::{Av1, H264, H265, Vp9};

/// Every encoder NewQ can send with, hardware first within each codec.
pub const ALL: &[Encoder] = &[
    enc("h264_nvenc", H264, Hw, "NVIDIA NVENC"),
    enc("h264_amf", H264, Hw, "AMD AMF"),
    enc("h264_qsv", H264, Hw, "Intel Quick Sync"),
    enc("h264_videotoolbox", H264, Hw, "Apple VideoToolbox"),
    enc("h264_mf", H264, Hw, "Windows Media Foundation"),
    enc("h264_d3d12va", H264, Hw, "Direct3D 12 (any graphics card)"),
    enc("h264_vaapi", H264, Hw, "VAAPI (Linux: Intel, AMD)"),
    enc("libx264", H264, Sw, "x264"),
    enc("libopenh264", H264, Sw, "OpenH264"),
    enc("hevc_nvenc", H265, Hw, "NVIDIA NVENC"),
    enc("hevc_amf", H265, Hw, "AMD AMF"),
    enc("hevc_qsv", H265, Hw, "Intel Quick Sync"),
    enc("hevc_videotoolbox", H265, Hw, "Apple VideoToolbox"),
    enc("hevc_mf", H265, Hw, "Windows Media Foundation"),
    enc("hevc_d3d12va", H265, Hw, "Direct3D 12 (any graphics card)"),
    enc("hevc_vaapi", H265, Hw, "VAAPI (Linux: Intel, AMD)"),
    enc("libx265", H265, Sw, "x265"),
    enc("libkvazaar", H265, Sw, "Kvazaar"),
    enc("av1_nvenc", Av1, Hw, "NVIDIA NVENC (RTX 40 and newer)"),
    enc("av1_amf", Av1, Hw, "AMD AMF (RDNA 3 and newer)"),
    enc("av1_qsv", Av1, Hw, "Intel Quick Sync (Arc and newer)"),
    enc("libsvtav1", Av1, Sw, "SVT-AV1"),
    enc("vp9_qsv", Vp9, Hw, "Intel Quick Sync"),
    enc("libvpx-vp9", Vp9, Sw, "libvpx VP9"),
];

/// The encoders for `codec`, in the order of `ALL`.
pub fn for_codec(codec: VideoCodec) -> impl Iterator<Item = &'static Encoder> {
    ALL.iter().filter(move |e| e.codec == codec)
}

/// The encoder called `name`, if NewQ knows it.
pub fn find(name: &str) -> Option<&'static Encoder> {
    ALL.iter().find(|e| e.name == name)
}

impl Encoder {
    /// Whether this kind of computer can have it at all: VAAPI is Linux's, VideoToolbox
    /// the Mac's, Media Foundation and Direct3D 12 Windows'.
    pub fn usable_here(&self) -> bool {
        let only = if self.name.ends_with("_vaapi") {
            Some("linux")
        } else if self.name.ends_with("_videotoolbox") {
            Some("macos")
        } else if self.name.ends_with("_mf") || self.name.ends_with("_d3d12va") {
            Some("windows")
        } else {
            None
        };
        only.is_none_or(|os| std::env::consts::OS == os)
    }

    /// "NVIDIA NVENC (hardware)".
    pub fn full_label(&self) -> String {
        format!(
            "{} ({})",
            self.label,
            match self.kind {
                Kind::Hardware => "hardware",
                Kind::Software => "software",
            }
        )
    }
}

/// Why `protocol` can't carry `codec`, if it can't. Found by publishing each to
/// a real MediaMTX: RTSP and RTMP carry all four (RTMP as "enhanced" RTMP for
/// the newer ones); SRT is MPEG-TS, which has H.264 and H.265 only.
pub fn protocol_problem(codec: VideoCodec, protocol: ServerProtocol) -> Option<String> {
    match (codec, protocol) {
        (VideoCodec::Av1 | VideoCodec::Vp9, ServerProtocol::Srt) => Some(format!(
            "SRT can carry H.264 and H.265 only, not {}: choose RTSP or RTMP for it, or another codec",
            codec.label()
        )),
        _ => None,
    }
}

/// What to know about who can watch `codec`, when it is not simply everyone.
pub fn viewer_note(codec: VideoCodec) -> Option<&'static str> {
    match codec {
        VideoCodec::H264 => None,
        VideoCodec::H265 => Some(
            "Most web browsers can't play H.265 in the web player (and so not from the QR \
             code). VLC, OBS, FFmpeg and most SRT players can.",
        ),
        VideoCodec::Av1 => Some(
            "AV1 in a web browser depends on the browser and the viewer's hardware. VLC, OBS \
             and FFmpeg play it. Needs a lot of the processor without a hardware AV1 encoder.",
        ),
        VideoCodec::Vp9 => Some(
            "VP9 plays in Chrome, Edge and Firefox; Safari is patchy. VLC, OBS and FFmpeg play \
             it. The software encoder uses a lot of the processor.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_encoder_has_a_name_a_label_and_a_codec_it_is_listed_under_once() {
        let mut seen = std::collections::HashSet::new();
        for e in ALL {
            assert!(seen.insert(e.name), "{} is listed twice", e.name);
            assert!(!e.label.is_empty(), "{} has no label", e.name);
            assert_eq!(find(e.name).map(|f| f.name), Some(e.name));
            // Named for what it makes.
            let family = match e.codec {
                H264 => "264",
                H265 => ["hevc", "265", "kvazaar"]
                    .iter()
                    .find(|k| e.name.contains(*k))
                    .copied()
                    .unwrap_or("?"),
                Av1 => "av1",
                Vp9 => "vp9",
            };
            assert!(
                e.name.contains(family),
                "{} is not a {} encoder",
                e.name,
                e.codec.label()
            );
        }
        for c in VideoCodec::ALL {
            assert!(
                for_codec(c).count() >= 2,
                "{} has too few encoders",
                c.label()
            );
            assert!(
                for_codec(c).any(|e| e.kind == Kind::Software),
                "{} has no software encoder",
                c.label()
            );
        }
        assert!(find("x264").is_none() && find("libx264").is_some());
        assert_eq!(
            find("h264_nvenc").unwrap().full_label(),
            "NVIDIA NVENC (hardware)"
        );
        assert_eq!(
            find("libopenh264").unwrap().full_label(),
            "OpenH264 (software)"
        );
    }

    #[test]
    fn every_computer_is_offered_a_software_encoder_for_each_codec_and_only_its_own_apis() {
        // OpenH264 is the software choice that is on every kind of computer.
        let h264: Vec<&str> = for_codec(H264)
            .filter(|e| e.usable_here())
            .map(|e| e.name)
            .collect();
        assert!(h264.contains(&"libopenh264") && h264.contains(&"h264_nvenc"));
        assert!(h264.contains(&"h264_amf") && h264.contains(&"h264_qsv"));
        for c in VideoCodec::ALL {
            assert!(
                for_codec(c).any(|e| e.usable_here() && e.kind == Kind::Software),
                "nothing in software for {}",
                c.label()
            );
        }
        // Each kind of computer has its own: not VAAPI on Windows, not Media Foundation on a Mac.
        let here = |n: &str| find(n).unwrap().usable_here();
        assert_eq!(here("h264_vaapi"), cfg!(target_os = "linux"));
        assert_eq!(here("h264_mf"), cfg!(windows));
        assert_eq!(here("hevc_d3d12va"), cfg!(windows));
        assert_eq!(here("h264_videotoolbox"), cfg!(target_os = "macos"));
        assert!(here("av1_nvenc") && here("libsvtav1"));
    }

    #[test]
    fn srt_cannot_carry_av1_or_vp9_but_everything_else_can_go_anywhere() {
        for codec in VideoCodec::ALL {
            for protocol in ServerProtocol::ALL {
                let problem = protocol_problem(codec, protocol);
                let srt_new = protocol == ServerProtocol::Srt
                    && matches!(codec, VideoCodec::Av1 | VideoCodec::Vp9);
                assert_eq!(problem.is_some(), srt_new, "{codec:?} over {protocol:?}");
            }
        }
        assert!(
            protocol_problem(VideoCodec::Av1, ServerProtocol::Srt)
                .unwrap()
                .contains("RTSP")
        );
        assert!(viewer_note(VideoCodec::H264).is_none());
        assert!(viewer_note(VideoCodec::H265).unwrap().contains("browser"));
    }
}
