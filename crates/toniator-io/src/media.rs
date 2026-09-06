//! Portable source-media ownership and strict container-2 entry manifests.

use super::*;
use toniator_domain::FrameRate;

/// Identifies the encoded source container or an explicitly ordered image sequence.
///
/// Video and animated images select the first video stream. Sequence entries may repeat a source
/// ID intentionally; ZIP entries remain unique and all bundled entries must be referenced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceMediaManifest {
    StillImage {
        source_id: SourceReferenceId,
    },
    AnimatedImage {
        source_id: SourceReferenceId,
    },
    Video {
        source_id: SourceReferenceId,
    },
    ImageSequence {
        source_ids: Vec<SourceReferenceId>,
        frame_rate: FrameRate,
    },
}

impl SourceMediaManifest {
    /// Returns authored source order; single-container media has exactly one entry.
    pub fn source_ids(&self) -> &[SourceReferenceId] {
        match self {
            Self::StillImage { source_id }
            | Self::AnimatedImage { source_id }
            | Self::Video { source_id } => std::slice::from_ref(source_id),
            Self::ImageSequence { source_ids, .. } => source_ids,
        }
    }

    /// Returns the first source ID used by the document's logical source reference.
    pub fn primary_source_id(&self) -> Option<&SourceReferenceId> {
        self.source_ids().first()
    }

    /// Validates exact bundle ownership and format compatibility without decoding source pixels.
    ///
    /// # Errors
    /// Rejects empty/excessive sequence order, missing/unused entries, and incompatible containers.
    pub(super) fn validate(&self, bundle: &SourceBundle) -> Result<(), SourceBundleError> {
        let ids = self.source_ids();
        if ids.is_empty() || ids.len() > 1_000_000 {
            return Err(SourceBundleError::new(
                "source.media.order",
                "media order must contain 1 through 1000000 frames",
            ));
        }
        let unique: HashSet<_> = ids.iter().collect();
        if unique.len() != bundle.len() || ids.iter().any(|id| bundle.get(id).is_none()) {
            return Err(SourceBundleError::new(
                "source.media.references",
                "media manifest must reference every bundled entry and no missing entry",
            ));
        }
        for id in ids {
            let format = bundle.get(id).expect("reference validated").format();
            let valid = match self {
                Self::StillImage { .. } | Self::ImageSequence { .. } => !matches!(
                    format,
                    EmbeddedSourceFormat::Video | EmbeddedSourceFormat::Gif
                ),
                Self::AnimatedImage { .. } => matches!(
                    format,
                    EmbeddedSourceFormat::Gif
                        | EmbeddedSourceFormat::Png
                        | EmbeddedSourceFormat::Webp
                        | EmbeddedSourceFormat::Avif
                ),
                Self::Video { .. } => format == EmbeddedSourceFormat::Video,
            };
            if !valid {
                return Err(SourceBundleError::new(
                    "source.media.format",
                    "encoded format is incompatible with its media kind",
                ));
            }
        }
        Ok(())
    }
}

/// Serializes media identity separately from reusable document configuration.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum MediaManifestDto {
    StillImage {
        source_id: String,
    },
    AnimatedImage {
        source_id: String,
    },
    Video {
        source_id: String,
        video_stream: u32,
    },
    ImageSequence {
        source_ids: Vec<String>,
        fps_numerator: u32,
        fps_denominator: u32,
    },
}

impl MediaManifestDto {
    /// Projects the exact ordered media manifest without copying document timing or End state.
    pub(super) fn from_media(media: &SourceMediaManifest) -> Self {
        match media {
            SourceMediaManifest::StillImage { source_id } => Self::StillImage {
                source_id: source_id.as_str().into(),
            },
            SourceMediaManifest::AnimatedImage { source_id } => Self::AnimatedImage {
                source_id: source_id.as_str().into(),
            },
            SourceMediaManifest::Video { source_id } => Self::Video {
                source_id: source_id.as_str().into(),
                video_stream: 0,
            },
            SourceMediaManifest::ImageSequence {
                source_ids,
                frame_rate,
            } => Self::ImageSequence {
                source_ids: source_ids.iter().map(|id| id.as_str().into()).collect(),
                fps_numerator: frame_rate.numerator(),
                fps_denominator: frame_rate.denominator(),
            },
        }
    }

    /// Reconstructs current media intent with exact rates and first-video-stream selection.
    ///
    /// # Errors
    /// Rejects invalid IDs/rates and unsupported video-stream selection; bundle validation follows.
    pub(super) fn into_media(self) -> Result<SourceMediaManifest, LoadError> {
        Ok(match self {
            Self::StillImage { source_id } => SourceMediaManifest::StillImage {
                source_id: dto_source_id(&source_id).map_err(domain_error)?,
            },
            Self::AnimatedImage { source_id } => SourceMediaManifest::AnimatedImage {
                source_id: dto_source_id(&source_id).map_err(domain_error)?,
            },
            Self::Video {
                source_id,
                video_stream,
            } => {
                if video_stream != 0 {
                    return Err(LoadError::DomainValidation {
                        context: "only the first video stream is supported".into(),
                    });
                }
                SourceMediaManifest::Video {
                    source_id: dto_source_id(&source_id).map_err(domain_error)?,
                }
            }
            Self::ImageSequence {
                source_ids,
                fps_numerator,
                fps_denominator,
            } => SourceMediaManifest::ImageSequence {
                source_ids: source_ids
                    .iter()
                    .map(|id| dto_source_id(id).map_err(domain_error))
                    .collect::<Result<_, _>>()?,
                frame_rate: FrameRate::new(fps_numerator, fps_denominator).map_err(domain_error)?,
            },
        })
    }
}

/// Generates index-based archive entry names; authored IDs and labels never become paths.
pub(super) fn entry_name(index: usize, format: EmbeddedSourceFormat) -> String {
    format!("sources/{index:06}.{}", format.extension())
}

/// Verifies canonical entries, aggregate bounds, hashes, ownership, and declared sequence order.
///
/// # Errors
/// Rejects missing/extra entries, noncanonical names, duplicate IDs, size/integrity failures,
/// incompatible media kinds, and invalid frame order before returning any source bundle.
pub(super) fn read_bundle(
    archive: &mut ZipArchive<File>,
    indices: &BTreeMap<String, usize>,
    manifests: Vec<SourceManifestDto>,
    media: MediaManifestDto,
) -> Result<SourceBundle, LoadError> {
    if manifests.is_empty()
        || manifests.len() != indices.len()
        || manifests.len() > MAX_SOURCE_ENTRIES
    {
        return Err(LoadError::EntryTopology {
            context: "source manifest must match every unique archive source entry".into(),
        });
    }
    let mut entries = Vec::with_capacity(manifests.len());
    let mut total = 0_u64;
    let mut prior_id = None;
    for (ordinal, manifest) in manifests.into_iter().enumerate() {
        let id = dto_source_id(&manifest.id).map_err(domain_error)?;
        validate_source_id(&id).map_err(|error| LoadError::EntryTopology {
            context: error.to_string(),
        })?;
        if prior_id.as_ref().is_some_and(|prior| prior >= &id) {
            return Err(LoadError::EntryTopology {
                context: "source entry manifest IDs must be unique and sorted".into(),
            });
        }
        prior_id = Some(id.clone());
        let format = manifest.format.into();
        if manifest.entry_name != entry_name(ordinal, format) {
            return Err(LoadError::EntryTopology {
                context: "source entry name is not canonical".into(),
            });
        }
        let index = indices
            .get(&manifest.entry_name)
            .ok_or_else(|| LoadError::EntryTopology {
                context: "manifest source entry is missing".into(),
            })?;
        let entry_size = archive
            .by_index_raw(*index)
            .map_err(|error| LoadError::Archive {
                context: error.to_string(),
            })?
            .size();
        total = total
            .checked_add(entry_size)
            .ok_or_else(|| LoadError::Limits {
                context: "aggregate source size overflowed".into(),
            })?;
        if total > MAX_SOURCE_BYTES || entry_size != manifest.byte_length {
            return Err(LoadError::Limits {
                context: "source lengths disagree or exceed the aggregate 128 MiB limit".into(),
            });
        }
        let bytes = read_limited(archive, *index, MAX_SOURCE_BYTES, &manifest.entry_name)?;
        if sha256_hex(&bytes) != manifest.sha256 {
            return Err(LoadError::Integrity {
                context: "source SHA-256 does not match manifest".into(),
            });
        }
        entries.push(
            EmbeddedSource::new(id, format, bytes, manifest.display_name).map_err(|error| {
                LoadError::EntryTopology {
                    context: error.to_string(),
                }
            })?,
        );
    }
    SourceBundle::new_media(entries, media.into_media()?).map_err(|error| {
        LoadError::EntryTopology {
            context: error.to_string(),
        }
    })
}
