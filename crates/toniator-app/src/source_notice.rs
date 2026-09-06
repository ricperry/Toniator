//! Session-only import disclosures; document timing and media metadata remain authoritative.

/// Retains source facts and dismissal independently of artwork history or serialized state.
#[derive(Clone, Debug, Default)]
pub(crate) struct SourceNotice {
    variable_frame_rate: bool,
    has_audio: bool,
    dismissed: bool,
    pub(crate) presented: bool,
}

impl SourceNotice {
    /// Captures already-probed facts without creating a second timing or source authority.
    pub(crate) fn new(metadata: &toniator_engine::SourceMediaMetadata) -> Self {
        Self {
            variable_frame_rate: metadata.variable_frame_rate,
            has_audio: metadata.has_audio,
            ..Self::default()
        }
    }

    /// Projects the current exact project rate and source disclosures until explicitly dismissed.
    pub(crate) fn message(&self, timing: &toniator_domain::ProjectTiming) -> Option<String> {
        if self.dismissed {
            return None;
        }
        let mut parts = Vec::new();
        if self.variable_frame_rate {
            let rate = timing.frame_rate();
            let rate = if rate.denominator() == 1 {
                rate.numerator().to_string()
            } else {
                format!("{}/{}", rate.numerator(), rate.denominator())
            };
            parts.push(format!(
                "Variable frame rate source. Project uses {rate} fps; adjust in Animation settings. Frames may repeat or be skipped."
            ));
        }
        if self.has_audio {
            parts.push("Source contains audio; exports are silent.".to_owned());
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    /// Dismisses only a currently presented source notice, preserving unseen notices behind errors.
    pub(crate) fn dismiss(&mut self) {
        if self.presented {
            self.dismissed = true;
        }
        self.presented = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks exact timing projection, combined notices and session-only dismissal through errors.
    ///
    /// # Panics
    /// Panics if current timing construction or notice/dismissal semantics regress.
    #[test]
    fn source_notice_tracks_current_rate_and_dismisses_only_its_own_message() {
        let mut document = toniator_domain::Document::new_default_document(
            toniator_domain::CanvasSpec {
                width: 10.0,
                height: 10.0,
            },
            toniator_domain::SourceReference::Unassigned,
        )
        .unwrap();
        let mut notice = SourceNotice {
            variable_frame_rate: true,
            has_audio: true,
            ..Default::default()
        };
        let original = notice.message(document.project_timing()).unwrap();
        assert!(original.contains("exports are silent"));
        assert!(original.contains("Frames may repeat or be skipped"));
        notice.dismiss();
        assert_eq!(
            notice.message(document.project_timing()),
            Some(original.clone())
        );
        notice.presented = true;
        notice.dismiss();
        assert_eq!(notice.message(document.project_timing()), None);
        notice.dismissed = false;
        notice.variable_frame_rate = false;
        assert_eq!(
            notice.message(document.project_timing()).unwrap(),
            "Source contains audio; exports are silent."
        );
        notice.has_audio = false;
        assert!(notice.message(document.project_timing()).is_none());
        notice.variable_frame_rate = true;
        let timing = toniator_domain::ProjectTiming::new(
            toniator_domain::FrameRate::new(30000, 1001).unwrap(),
            document.project_timing().frame_range(),
        );
        document = document
            .with_temporal_authority(timing, Vec::new())
            .unwrap();
        assert!(
            notice
                .message(document.project_timing())
                .unwrap()
                .contains("30000/1001 fps")
        );
    }
}
