use super::mkv::Track;
use serde::{Deserialize, Deserializer, de::Error as _};

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FilterValue {
    Bool(bool),
    Text(String),
    Languages(Vec<String>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OutputDirValue {
    Disabled(bool),
    Path(String),
}

fn deserialize_output_dir<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match OutputDirValue::deserialize(deserializer)? {
        OutputDirValue::Disabled(false) => Ok(None),
        OutputDirValue::Disabled(true) => {
            Err(D::Error::custom("OutputDir must be false or a path string"))
        }
        OutputDirValue::Path(path) => Ok(Some(path)),
    }
}

impl Default for FilterValue {
    fn default() -> Self {
        Self::Bool(false)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrackFilter {
    pub enabled: bool,
    pub languages: Vec<String>,
}

impl TrackFilter {
    pub fn from_value(value: &FilterValue) -> Self {
        match value {
            FilterValue::Bool(enabled) => Self {
                enabled: *enabled,
                languages: Vec::new(),
            },
            FilterValue::Text(text) => {
                let text = text.trim();
                if text.is_empty() || text.eq_ignore_ascii_case("false") {
                    Self::default()
                } else if text.eq_ignore_ascii_case("true") {
                    Self {
                        enabled: true,
                        languages: Vec::new(),
                    }
                } else {
                    Self {
                        enabled: true,
                        languages: vec![text.to_owned()],
                    }
                }
            }
            FilterValue::Languages(languages) => {
                let languages = languages
                    .iter()
                    .map(|language| language.trim().to_owned())
                    .filter(|language| !language.is_empty())
                    .collect::<Vec<_>>();
                Self {
                    enabled: !languages.is_empty(),
                    languages,
                }
            }
        }
    }

    pub fn matches_track(&self, track: &Track) -> bool {
        if !self.enabled {
            return false;
        }
        if self.languages.is_empty() {
            return true;
        }

        let language = track
            .properties
            .as_ref()
            .and_then(|properties| properties.effective_language())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();

        self.languages.iter().any(|target| {
            let target = target.to_ascii_lowercase();
            language == target
                || (!language.is_empty() && language.starts_with(&format!("{target}-")))
                || (!language.is_empty() && target.starts_with(&format!("{language}-")))
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    #[serde(
        rename = "ArchiveMode",
        alias = "RemuxMode",
        alias = "CompactMode",
        default
    )]
    pub archive_mode: bool,
    #[serde(rename = "ExtractVideo", default)]
    pub extract_video: FilterValue,
    #[serde(rename = "ExtractAudio", default)]
    pub extract_audio: FilterValue,
    #[serde(rename = "ExtractSubtitles", default)]
    pub extract_subtitles: FilterValue,
    #[serde(rename = "ExtractChapters", default)]
    pub extract_chapters: FilterValue,
    #[serde(rename = "ExtractAttachments", default)]
    pub extract_attachments: FilterValue,
    #[serde(
        rename = "OutputDir",
        default,
        deserialize_with = "deserialize_output_dir"
    )]
    pub output_dir: Option<String>,
}

impl Profile {
    pub fn video_filter(&self) -> TrackFilter {
        TrackFilter::from_value(&self.extract_video)
    }

    pub fn audio_filter(&self) -> TrackFilter {
        TrackFilter::from_value(&self.extract_audio)
    }

    pub fn subtitle_filter(&self) -> TrackFilter {
        TrackFilter::from_value(&self.extract_subtitles)
    }

    pub fn chapters_enabled(&self) -> bool {
        TrackFilter::from_value(&self.extract_chapters).enabled
    }

    pub fn attachments_enabled(&self) -> bool {
        TrackFilter::from_value(&self.extract_attachments).enabled
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Patterns {
    #[serde(rename = "VideoTracks", default = "default_video_pattern")]
    pub video_tracks: String,
    #[serde(rename = "AudioTracks", default = "default_audio_pattern")]
    pub audio_tracks: String,
    #[serde(rename = "SubtitleTracks", default = "default_subtitle_pattern")]
    pub subtitle_tracks: String,
    #[serde(rename = "Chapters", default = "default_chapters_pattern")]
    pub chapters: String,
    #[serde(rename = "Attachments", default = "default_attachments_pattern")]
    pub attachments: String,
}

impl Default for Patterns {
    fn default() -> Self {
        Self {
            video_tracks: default_video_pattern(),
            audio_tracks: default_audio_pattern(),
            subtitle_tracks: default_subtitle_pattern(),
            chapters: default_chapters_pattern(),
            attachments: default_attachments_pattern(),
        }
    }
}

fn default_video_pattern() -> String {
    "{FilenameNoExt}.Track{TrackID:00}.{Language}".to_owned()
}
fn default_audio_pattern() -> String {
    "{FilenameNoExt}.Track{TrackID:00}.{Language}".to_owned()
}
fn default_subtitle_pattern() -> String {
    "{FilenameNoExt}.Track{TrackID:00}.{Language}.{Flags}".to_owned()
}
fn default_chapters_pattern() -> String {
    "Chapters/{FilenameNoExt}".to_owned()
}
fn default_attachments_pattern() -> String {
    "Attachments/{AttachmentFilename}".to_owned()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AppConfig {
    #[serde(rename = "MKVToolNixDir", default)]
    pub mkv_tool_nix_dir: String,
    #[serde(default)]
    pub patterns: Patterns,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, TrackFilter};
    use crate::domain::mkv::{Track, TrackProperties};

    fn filter(language: &str) -> TrackFilter {
        TrackFilter {
            enabled: true,
            languages: vec![language.to_owned()],
        }
    }

    #[test]
    fn prefers_ietf_language_without_converting_the_legacy_value() {
        let track = Track {
            properties: Some(TrackProperties {
                language: Some("por".to_owned()),
                language_ietf: Some("pt-BR".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(filter("pt-BR").matches_track(&track));
        assert!(!filter("por").matches_track(&track));
    }

    #[test]
    fn uses_legacy_language_when_ietf_language_is_missing() {
        let track = Track {
            properties: Some(TrackProperties {
                language: Some("por".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };

        assert!(filter("por").matches_track(&track));
        assert!(!filter("pt").matches_track(&track));
    }

    #[test]
    fn output_dir_accepts_false_or_a_path() {
        let disabled: AppConfig = toml::from_str("[[profiles]]\nOutputDir = false").unwrap();
        assert_eq!(disabled.profiles[0].output_dir, None);

        let path: AppConfig =
            toml::from_str("[[profiles]]\nOutputDir = \"/tmp/extracted\"").unwrap();
        assert_eq!(
            path.profiles[0].output_dir.as_deref(),
            Some("/tmp/extracted")
        );

        assert!(toml::from_str::<AppConfig>("[[profiles]]\nOutputDir = true").is_err());
    }
}
