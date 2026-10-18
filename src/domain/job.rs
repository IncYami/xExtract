use super::{AppConfig, MkvInfo, Profile, Track};
use crate::error::AppError;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ExtractJob {
    pub input_file: PathBuf,
    pub output_dir: PathBuf,
    pub track_ids: Vec<u32>,
    pub output_files: Vec<String>,
    pub extract_chapters: Option<String>,
    pub attachment_ids: Vec<u32>,
    pub attachment_files: Vec<String>,
    pub is_archive_mode: bool,
    pub archive_output_file: Option<String>,
    pub video_track_ids: Vec<u32>,
    pub audio_track_ids: Vec<u32>,
    pub subtitle_track_ids: Vec<u32>,
}

impl ExtractJob {
    pub fn has_work(&self) -> bool {
        self.is_archive_mode
            || !self.track_ids.is_empty()
            || self.extract_chapters.is_some()
            || !self.attachment_ids.is_empty()
    }
}

pub fn create_job(
    file_path: &Path,
    info: &MkvInfo,
    profile: &Profile,
    config: &AppConfig,
) -> Result<ExtractJob, AppError> {
    let output_dir = profile
        .output_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| file_path.parent().map(Path::to_owned).unwrap_or_default());

    let archive_output_file = profile.archive_mode.then(|| {
        let filename = file_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");
        let safe_profile = profile.name.replace(' ', "");
        format!("{filename}.{safe_profile}.mkv")
    });

    let mut job = ExtractJob {
        input_file: file_path.to_owned(),
        output_dir,
        is_archive_mode: profile.archive_mode,
        archive_output_file,
        ..Default::default()
    };

    let video_filter = profile.video_filter();
    let audio_filter = profile.audio_filter();
    let subtitle_filter = profile.subtitle_filter();

    for track in &info.tracks {
        let track_kind = track.kind.as_deref().unwrap_or_default();
        let (matches, pattern) = match track_kind {
            "video" => (
                video_filter.matches_track(track),
                &config.patterns.video_tracks,
            ),
            "audio" => (
                audio_filter.matches_track(track),
                &config.patterns.audio_tracks,
            ),
            "subtitles" => (
                subtitle_filter.matches_track(track),
                &config.patterns.subtitle_tracks,
            ),
            _ => (false, &config.patterns.video_tracks),
        };

        if !matches {
            continue;
        }

        let track_id = track
            .id
            .ok_or_else(|| AppError::message("MKV track is missing its id"))?;
        let extension = extension_for_track(track);
        let output_name = format!(
            "{}.{}",
            resolve_pattern(pattern, file_path, Some(track), None)?,
            extension
        );
        job.track_ids.push(track_id);
        job.output_files.push(output_name);

        match track_kind {
            "video" => job.video_track_ids.push(track_id),
            "audio" => job.audio_track_ids.push(track_id),
            "subtitles" => job.subtitle_track_ids.push(track_id),
            _ => {}
        }
    }

    if profile.chapters_enabled() && !info.chapters.is_empty() {
        let value = resolve_pattern(&config.patterns.chapters, file_path, None, None)?;
        job.extract_chapters = Some(if value.to_ascii_lowercase().ends_with(".xml") {
            value
        } else {
            format!("{value}.xml")
        });
    }

    if profile.attachments_enabled() && !info.attachments.is_empty() {
        for attachment in &info.attachments {
            let attachment_id = attachment
                .id
                .ok_or_else(|| AppError::message("MKV attachment is missing its id"))?;
            let attachment_filename = attachment
                .file_name
                .as_deref()
                .ok_or_else(|| AppError::message("MKV attachment is missing its file name"))?;
            job.attachment_ids.push(attachment_id);
            job.attachment_files.push(resolve_pattern(
                &config.patterns.attachments,
                file_path,
                None,
                Some(attachment_filename),
            )?);
        }
    }

    Ok(job)
}

fn resolve_pattern(
    pattern: &str,
    file_path: &Path,
    track: Option<&Track>,
    attachment_filename: Option<&str>,
) -> Result<String, AppError> {
    let filename = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let filename_no_ext = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    let mut result = pattern
        .replace("{Filename}", filename)
        .replace("{FilenameNoExt}", filename_no_ext);

    if let Some(track) = track {
        let track_id = track
            .id
            .ok_or_else(|| AppError::message("MKV track is missing its id"))?;
        result = replace_track_id(result, track_id);
        result = result.replace("{TrackID}", &track_id.to_string());

        let language = track
            .properties
            .as_ref()
            .and_then(|properties| properties.effective_language());
        result = match language {
            Some(language) => result.replace("{Language}", language),
            None => result.replace(".{Language}", "").replace("{Language}", ""),
        };

        let mut flags = Vec::new();
        if track
            .properties
            .as_ref()
            .and_then(|properties| properties.forced_track)
            .unwrap_or(false)
        {
            flags.push("Forced");
        }
        if track
            .properties
            .as_ref()
            .and_then(|properties| properties.flag_hearing_impaired)
            .unwrap_or(false)
        {
            flags.push("SDH");
        }
        if flags.is_empty() {
            result = result.replace(".{Flags}", "").replace("{Flags}", "");
        } else {
            result = result.replace("{Flags}", &flags.join("."));
        }
    }

    if let Some(filename) = attachment_filename {
        result = result.replace("{AttachmentFilename}", filename);
    }

    Ok(result)
}

fn replace_track_id(mut value: String, id: u32) -> String {
    while let Some(start) = value.find("{TrackID:") {
        let Some(relative_end) = value[start..].find('}') else {
            break;
        };
        let end = start + relative_end + 1;
        let width = &value[start + "{TrackID:".len()..end - 1];
        if width.is_empty() || !width.chars().all(|character| character.is_ascii_digit()) {
            break;
        }
        let replacement = format!("{id:0width$}", width = width.len());
        value.replace_range(start..end, &replacement);
    }
    value
}

fn extension_for_track(track: &Track) -> &'static str {
    let codec = track
        .properties
        .as_ref()
        .and_then(|properties| properties.codec_id.as_deref())
        .unwrap_or_default()
        .to_ascii_uppercase();
    match track.kind.as_deref().unwrap_or_default() {
        "video" => {
            if codec.contains("V_MS/VFW/FOURCC") {
                "avi"
            } else if codec.contains("V_UNCOMPRESSED") {
                "raw"
            } else if codec.contains("V_MPEG4/ISO/") {
                "avc"
            } else if codec.contains("V_MPEGH/ISO/HEVC") {
                "hevc"
            } else if codec.contains("V_AV1") {
                "av1"
            } else if codec.contains("V_MPEG4/MS/V3") {
                "mp4"
            } else if codec.contains("V_MPEG1") || codec.contains("V_MPEG2") {
                "mpg"
            } else if codec.contains("V_REAL/") {
                "rm"
            } else if codec.contains("V_QUICKTIME") || codec.contains("V_PRORES") {
                "mov"
            } else if codec.contains("V_THEORA") {
                "ogv"
            } else if codec.contains("V_VP") {
                "ivf"
            } else if codec.contains("V_DIRAC") {
                "drc"
            } else {
                "mkv"
            }
        }
        "audio" => {
            if codec.contains("A_MPEG/L3") {
                "mp3"
            } else if codec.contains("A_MPEG/L2") {
                "mp2"
            } else if codec.contains("A_MPEG/L1") {
                "mpa"
            } else if codec.contains("A_PCM") || codec.contains("A_MS/ACM") {
                "wav"
            } else if codec.contains("A_MPC") {
                "mpc"
            } else if codec.contains("A_AC3") {
                "ac3"
            } else if codec.contains("A_EAC3") {
                "eac3"
            } else if codec.contains("A_ALAC") {
                "caf"
            } else if codec.contains("A_DTS") {
                "dts"
            } else if codec.contains("A_VORBIS") {
                "ogg"
            } else if codec.contains("A_FLAC") {
                "flac"
            } else if codec.contains("A_REAL") {
                "ra"
            } else if codec.contains("A_AAC") {
                "aac"
            } else if codec.contains("A_QUICKTIME") {
                "mov"
            } else if codec.contains("A_TRUEHD") {
                "thd"
            } else if codec.contains("A_TTA1") {
                "tta"
            } else if codec.contains("A_WAVPACK4") {
                "wv"
            } else if codec.contains("A_OPUS") {
                "opus"
            } else if codec.contains("A_MLP") {
                "mlp"
            } else {
                "mka"
            }
        }
        "subtitles" => {
            if codec.contains("S_TEXT/UTF8") || codec.contains("S_TEXT/ASCII") {
                "srt"
            } else if codec.contains("S_TEXT/SSA") || codec.contains("S_TEXT/ASS") {
                "ass"
            } else if codec.contains("S_TEXT/USF") {
                "usf"
            } else if codec.contains("S_TEXT/WEBVTT") {
                "webvtt"
            } else if codec.contains("S_IMAGE/BMP") || codec.contains("S_VOBSUB") {
                "sub"
            } else if codec.contains("S_DVBSUB") {
                "dvbsub"
            } else if codec.contains("S_HDMV/PGS") {
                "sup"
            } else if codec.contains("S_HDMV/TEXTST") {
                "textst"
            } else if codec.contains("S_KATE") {
                "ogg"
            } else {
                "mks"
            }
        }
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::replace_track_id;

    #[test]
    fn formats_track_id_with_zero_padding() {
        assert_eq!(replace_track_id("track-{TrackID:00}".into(), 3), "track-03");
        assert_eq!(
            replace_track_id("track-{TrackID:000}".into(), 12),
            "track-012"
        );
    }
}
