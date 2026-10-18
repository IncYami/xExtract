#![allow(dead_code)]

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Attachment {
    #[serde(rename = "content_type", default)]
    pub content_type: Option<String>,
    #[serde(rename = "description", default)]
    pub description: Option<String>,
    #[serde(rename = "file_name", default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Chapter {
    #[serde(rename = "num_entries", default)]
    pub num_entries: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MkvContainerProperties {
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(rename = "muxing_application", default)]
    pub muxing_application: Option<String>,
    #[serde(rename = "writing_application", default)]
    pub writing_application: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MkvContainer {
    #[serde(default)]
    pub properties: Option<MkvContainerProperties>,
    #[serde(default)]
    pub recognized: Option<bool>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TrackProperties {
    #[serde(rename = "codec_id", default)]
    pub codec_id: Option<String>,
    #[serde(rename = "default_track", default)]
    pub default_track: Option<bool>,
    #[serde(rename = "forced_track", default)]
    pub forced_track: Option<bool>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(rename = "language_ietf", default)]
    pub language_ietf: Option<String>,
    #[serde(rename = "track_name", default)]
    pub track_name: Option<String>,
    #[serde(rename = "pixel_dimensions", default)]
    pub pixel_dimensions: Option<String>,
    #[serde(rename = "audio_channels", default)]
    pub audio_channels: Option<u32>,
    #[serde(rename = "audio_sampling_frequency", default)]
    pub audio_sampling_frequency: Option<f64>,
    #[serde(rename = "flag_hearing_impaired", default)]
    pub flag_hearing_impaired: Option<bool>,
    #[serde(rename = "text_subtitles", default)]
    pub text_subtitles: Option<bool>,
}

impl TrackProperties {
    /// Returns the language exactly as reported by MKVToolNix.
    ///
    /// Matroska files may contain both the IETF language tag and the legacy
    /// language field. The IETF value has precedence; the legacy value is
    /// used only when the IETF field is missing.
    pub fn effective_language(&self) -> Option<&str> {
        if let Some(language_ietf) = self.language_ietf.as_deref()
            && !language_ietf.trim().is_empty()
        {
            return Some(language_ietf);
        }
        self.language
            .as_deref()
            .filter(|language| !language.trim().is_empty())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Track {
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub properties: Option<TrackProperties>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MkvInfo {
    /// Complete metadata returned by `mkvmerge -J`.
    #[serde(skip)]
    pub raw: Box<serde_json::Value>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    #[serde(default)]
    pub container: MkvContainer,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

impl MkvInfo {
    pub fn from_raw(raw: serde_json::Value) -> Result<Self, serde_json::Error> {
        let mut info: Self = serde_json::from_value(raw.clone())?;
        info.raw = Box::new(raw);
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::MkvInfo;
    use serde_json::json;

    #[test]
    fn preserves_unknown_metadata_in_raw_json() {
        let raw = json!({
            "identification_format_version": 21,
            "tracks": [{
                "id": 1,
                "type": "audio",
                "properties": {
                    "language_ietf": "pt-BR",
                    "future_property": "kept"
                }
            }],
            "future_top_level_property": { "value": true }
        });

        let info = MkvInfo::from_raw(raw.clone()).expect("valid mkvmerge metadata");

        assert_eq!(*info.raw, raw);
        assert_eq!(info.tracks[0].id, Some(1));
        assert_eq!(
            info.tracks[0]
                .properties
                .as_ref()
                .and_then(|properties| properties.language_ietf.as_deref()),
            Some("pt-BR")
        );
    }
}
