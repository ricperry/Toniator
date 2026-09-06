//! Project timing and simple Start-to-End transition authority.

use super::*;

/// The exact scalar fields permitted to carry End-frame overrides.
pub const ANIMATABLE_SCALAR_FIELD_IDS: &[PropertyFieldId] = &[
    PropertyFieldId::Density,
    PropertyFieldId::DensityAspect,
    PropertyFieldId::RotationDegrees,
    PropertyFieldId::TranslationX,
    PropertyFieldId::TranslationY,
    PropertyFieldId::MarkMinimumFill,
    PropertyFieldId::MarkMaximumFill,
    PropertyFieldId::ConnectedMinimumThickness,
    PropertyFieldId::ConnectedMaximumThickness,
    PropertyFieldId::CurveResponseBias,
    PropertyFieldId::ShapeRotationDegrees,
    PropertyFieldId::ModeledMappingGain,
    PropertyFieldId::ModeledMappingBias,
    PropertyFieldId::ColorRed,
    PropertyFieldId::ColorGreen,
    PropertyFieldId::ColorBlue,
    PropertyFieldId::ColorAlpha,
    PropertyFieldId::Opacity,
    PropertyFieldId::RegionMinimumFill,
    PropertyFieldId::RegionMaximumFill,
];

/// Explains why an active property descriptor cannot receive an End-frame override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporalStaticReason {
    /// The setting is discrete or otherwise excluded from the first temporal contract.
    StaticSetting,
    /// The field belongs to a structural definition rather than effective channel state.
    StructuralDefinition,
    /// This field is continuous but the addressed target does not own temporal authority.
    UnsupportedTarget,
}

/// Declares the temporal editing mode exposed by one exact property descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporalCapability {
    /// The field accepts one scalar End value and easing mode.
    Scalar,
    /// A solid-paint component also participates in grouped color End authoring.
    ScalarAndGroupedColor,
    /// The descriptor remains static for the complete render job.
    Static(TemporalStaticReason),
}

/// Derives target-sensitive temporal capability from the exhaustive scalar allowlist.
pub const fn temporal_capability(
    field: PropertyFieldId,
    target: PropertyTarget,
) -> TemporalCapability {
    if !is_animatable_scalar_field(field) {
        return TemporalCapability::Static(match target {
            PropertyTarget::Definition(_)
            | PropertyTarget::Mechanism(_, _)
            | PropertyTarget::OutputLayer(_, _)
            | PropertyTarget::GuideDimension(_, _, _) => TemporalStaticReason::StructuralDefinition,
            PropertyTarget::Document
            | PropertyTarget::Channel(_)
            | PropertyTarget::ChannelOutput(_, _) => TemporalStaticReason::StaticSetting,
        });
    }
    let valid_target = match field {
        PropertyFieldId::Density
        | PropertyFieldId::DensityAspect
        | PropertyFieldId::RotationDegrees
        | PropertyFieldId::ShapeRotationDegrees => {
            matches!(
                target,
                PropertyTarget::Document | PropertyTarget::Channel(_)
            )
        }
        PropertyFieldId::TranslationX
        | PropertyFieldId::TranslationY
        | PropertyFieldId::ModeledMappingGain
        | PropertyFieldId::ModeledMappingBias
        | PropertyFieldId::ColorRed
        | PropertyFieldId::ColorGreen
        | PropertyFieldId::ColorBlue
        | PropertyFieldId::ColorAlpha
        | PropertyFieldId::Opacity => matches!(target, PropertyTarget::Channel(_)),
        PropertyFieldId::MarkMinimumFill
        | PropertyFieldId::MarkMaximumFill
        | PropertyFieldId::ConnectedMinimumThickness
        | PropertyFieldId::ConnectedMaximumThickness
        | PropertyFieldId::CurveResponseBias
        | PropertyFieldId::RegionMinimumFill
        | PropertyFieldId::RegionMaximumFill => {
            matches!(target, PropertyTarget::ChannelOutput(_, _))
        }
        _ => false,
    };
    if !valid_target {
        return TemporalCapability::Static(match target {
            PropertyTarget::OutputLayer(_, _)
            | PropertyTarget::Definition(_)
            | PropertyTarget::Mechanism(_, _)
            | PropertyTarget::GuideDimension(_, _, _) => TemporalStaticReason::StructuralDefinition,
            _ => TemporalStaticReason::UnsupportedTarget,
        });
    }
    if matches!(
        field,
        PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
    ) {
        TemporalCapability::ScalarAndGroupedColor
    } else {
        TemporalCapability::Scalar
    }
}

/// Reports membership in the exact 20-field scalar transition inventory.
pub const fn is_animatable_scalar_field(field: PropertyFieldId) -> bool {
    matches!(
        field,
        PropertyFieldId::Density
            | PropertyFieldId::DensityAspect
            | PropertyFieldId::RotationDegrees
            | PropertyFieldId::TranslationX
            | PropertyFieldId::TranslationY
            | PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::ConnectedMinimumThickness
            | PropertyFieldId::ConnectedMaximumThickness
            | PropertyFieldId::CurveResponseBias
            | PropertyFieldId::ShapeRotationDegrees
            | PropertyFieldId::ModeledMappingGain
            | PropertyFieldId::ModeledMappingBias
            | PropertyFieldId::ColorRed
            | PropertyFieldId::ColorGreen
            | PropertyFieldId::ColorBlue
            | PropertyFieldId::ColorAlpha
            | PropertyFieldId::Opacity
            | PropertyFieldId::RegionMinimumFill
            | PropertyFieldId::RegionMaximumFill
    )
}

/// A reduced positive rational constant frame rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRate {
    numerator: u32,
    denominator: u32,
}

impl FrameRate {
    /// Constructs and reduces an exact positive frame rate.
    ///
    /// # Errors
    ///
    /// Returns `temporal.frame_rate` when either integer is zero.
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, ValidationError> {
        if numerator == 0 || denominator == 0 {
            return Err(ValidationError::new(
                "temporal.frame_rate",
                "frame rate numerator and denominator must be positive",
            ));
        }
        let divisor = gcd_u64(u64::from(numerator), u64::from(denominator)) as u32;
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    /// Returns the reduced frames-per-second numerator.
    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    /// Returns the reduced frames-per-second denominator.
    pub const fn denominator(self) -> u32 {
        self.denominator
    }

    /// Returns the exact output time for a zero-based local frame offset.
    ///
    /// # Errors
    ///
    /// Returns `temporal.frame_time` when multiplying the frame offset by the
    /// rate denominator exceeds the supported `u64` rational-time numerator.
    pub fn time_for_frame(self, frame_offset: u64) -> Result<RationalTime, ValidationError> {
        let numerator = frame_offset
            .checked_mul(u64::from(self.denominator))
            .ok_or_else(|| {
                ValidationError::new(
                    "temporal.frame_time",
                    "frame time exceeds the supported exact integer range",
                )
            })?;
        RationalTime::new(numerator, u64::from(self.numerator))
    }
}

impl Default for FrameRate {
    /// Supplies the initial still-animation proposal of exactly 30 frames per second.
    fn default() -> Self {
        Self {
            numerator: 30,
            denominator: 1,
        }
    }
}

/// One reduced nonnegative rational time in seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RationalTime {
    numerator: u64,
    denominator: u64,
}

impl Default for RationalTime {
    /// Supplies exact zero seconds with the invariant-preserving denominator one.
    fn default() -> Self {
        Self {
            numerator: 0,
            denominator: 1,
        }
    }
}

impl RationalTime {
    /// Constructs and reduces an exact nonnegative rational time.
    ///
    /// # Errors
    ///
    /// Returns `temporal.time` when the denominator is zero.
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, ValidationError> {
        if denominator == 0 {
            return Err(ValidationError::new(
                "temporal.time",
                "rational time denominator must be positive",
            ));
        }
        let divisor = gcd_u64(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    /// Returns the reduced time numerator.
    pub const fn numerator(self) -> u64 {
        self.numerator
    }

    /// Returns the reduced time denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Compares two exact rational times without floating-point conversion.
    pub fn checked_cmp(self, other: Self) -> std::cmp::Ordering {
        (u128::from(self.numerator) * u128::from(other.denominator))
            .cmp(&(u128::from(other.numerator) * u128::from(self.denominator)))
    }

    /// Adds exact nonnegative seconds and reduces the result without floating-point rounding.
    ///
    /// # Errors
    /// Rejects sums whose intermediate or reduced representation exceeds the integer bounds.
    pub fn checked_add(self, other: Self) -> Result<Self, ValidationError> {
        let overflow = || ValidationError::new("temporal.time", "exact time addition overflowed");
        let shared = gcd_u64(self.denominator, other.denominator);
        let numerator = (u128::from(self.numerator) * u128::from(other.denominator / shared))
            .checked_add(u128::from(other.numerator) * u128::from(self.denominator / shared))
            .ok_or_else(overflow)?;
        let denominator = u128::from(self.denominator / shared) * u128::from(other.denominator);
        let divisor = gcd_u128(numerator, denominator);
        Self::new(
            u64::try_from(numerator / divisor).map_err(|_| overflow())?,
            u64::try_from(denominator / divisor).map_err(|_| overflow())?,
        )
    }
}

/// A nonempty half-open source-time range represented by exact rationals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeRange {
    start: RationalTime,
    end: RationalTime,
}

impl TimeRange {
    /// Constructs one exact nonempty half-open source-time range.
    ///
    /// # Errors
    ///
    /// Returns `temporal.time_range` unless `end` is strictly after `start`.
    pub fn new(start: RationalTime, end: RationalTime) -> Result<Self, ValidationError> {
        if end.checked_cmp(start) != std::cmp::Ordering::Greater {
            return Err(ValidationError::new(
                "temporal.time_range",
                "source time range must be nonempty and half-open",
            ));
        }
        Ok(Self { start, end })
    }

    /// Returns the included source-time boundary.
    pub const fn start(self) -> RationalTime {
        self.start
    }

    /// Returns the excluded source-time boundary.
    pub const fn end(self) -> RationalTime {
        self.end
    }

    /// Returns the exact range duration after checked integer reduction.
    ///
    /// # Errors
    ///
    /// Returns `temporal.time_range.duration` if the reduced rational does not
    /// fit the supported `u64` numerator and denominator representation.
    pub fn duration(self) -> Result<RationalTime, ValidationError> {
        let left = u128::from(self.end.numerator) * u128::from(self.start.denominator);
        let right = u128::from(self.start.numerator) * u128::from(self.end.denominator);
        let numerator = left - right;
        let denominator = u128::from(self.end.denominator) * u128::from(self.start.denominator);
        let divisor = gcd_u128(numerator, denominator);
        let numerator = u64::try_from(numerator / divisor).map_err(|_| {
            ValidationError::new(
                "temporal.time_range.duration",
                "source range duration numerator exceeds the supported exact integer range",
            )
        })?;
        let denominator = u64::try_from(denominator / divisor).map_err(|_| {
            ValidationError::new(
                "temporal.time_range.duration",
                "source range duration denominator exceeds the supported exact integer range",
            )
        })?;
        RationalTime::new(numerator, denominator)
    }
}

/// A nonempty half-open range of absolute output frame indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRange {
    start: u64,
    end_exclusive: u64,
}

impl FrameRange {
    /// Constructs one exact nonempty half-open frame range.
    ///
    /// # Errors
    ///
    /// Returns `temporal.frame_range` when `end_exclusive` does not exceed `start`.
    pub fn new(start: u64, end_exclusive: u64) -> Result<Self, ValidationError> {
        if end_exclusive <= start {
            return Err(ValidationError::new(
                "temporal.frame_range",
                "frame range must be nonempty and half-open",
            ));
        }
        Ok(Self {
            start,
            end_exclusive,
        })
    }

    /// Returns the first included absolute output frame.
    pub const fn start(self) -> u64 {
        self.start
    }

    /// Returns the first excluded absolute output frame.
    pub const fn end_exclusive(self) -> u64 {
        self.end_exclusive
    }

    /// Returns the exact number of frames in this already-validated range.
    pub const fn frame_count(self) -> u64 {
        self.end_exclusive - self.start
    }

    /// Returns the exact local offset for one included absolute frame.
    ///
    /// # Errors
    ///
    /// Returns `temporal.frame` when the frame lies outside this range.
    pub fn local_offset(self, frame: u64) -> Result<u64, ValidationError> {
        if frame < self.start || frame >= self.end_exclusive {
            return Err(ValidationError::new(
                "temporal.frame",
                "requested frame lies outside the selected half-open range",
            ));
        }
        Ok(frame - self.start)
    }

    /// Returns clamped animation progress, using zero for a one-frame job.
    ///
    /// # Errors
    ///
    /// Returns the same out-of-range diagnostic as `local_offset`.
    pub fn progress(self, frame: u64) -> Result<f64, ValidationError> {
        let offset = self.local_offset(frame)?;
        if self.frame_count() == 1 {
            return Ok(0.0);
        }
        Ok(offset as f64 / (self.frame_count() - 1) as f64)
    }
}

impl Default for FrameRange {
    /// Supplies one output frame at absolute index zero.
    fn default() -> Self {
        Self {
            start: 0,
            end_exclusive: 1,
        }
    }
}

/// Project-only output timing, excluded from reusable `DocumentConfiguration`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectTiming {
    frame_rate: FrameRate,
    frame_range: FrameRange,
    source_time_range: Option<TimeRange>,
}

impl ProjectTiming {
    /// Constructs project timing from validated exact frame-rate and range values.
    pub const fn new(frame_rate: FrameRate, frame_range: FrameRange) -> Self {
        Self {
            frame_rate,
            frame_range,
            source_time_range: None,
        }
    }

    /// Attaches an exact selected source-time range without changing output timing.
    pub const fn with_source_time_range(mut self, source_time_range: TimeRange) -> Self {
        self.source_time_range = Some(source_time_range);
        self
    }

    /// Returns the exact constant output frame rate.
    pub const fn frame_rate(&self) -> FrameRate {
        self.frame_rate
    }

    /// Returns the selected half-open output frame range.
    pub const fn frame_range(&self) -> FrameRange {
        self.frame_range
    }

    /// Returns the optional exact selected source-time interval.
    pub const fn source_time_range(&self) -> Option<TimeRange> {
        self.source_time_range
    }

    /// Returns the exact local output time for one included absolute frame.
    ///
    /// # Errors
    ///
    /// Returns range or integer-overflow diagnostics without rounding time.
    pub fn time_for_frame(&self, frame: u64) -> Result<RationalTime, ValidationError> {
        self.frame_rate
            .time_for_frame(self.frame_range.local_offset(frame)?)
    }

    /// Maps an included output frame to source time without changing source playback speed.
    ///
    /// Frame-selected ranges use absolute indices. An explicit source interval starts at its
    /// authored time and advances by local output offsets, retaining its half-open end boundary.
    ///
    /// # Errors
    /// Rejects frames outside the output/source interval and exact-time arithmetic overflow.
    pub fn source_time_for_frame(&self, frame: u64) -> Result<RationalTime, ValidationError> {
        let local = self.time_for_frame(frame)?;
        match self.source_time_range {
            Some(range) => {
                let time = range.start().checked_add(local)?;
                if time.checked_cmp(range.end()) != std::cmp::Ordering::Less {
                    return Err(ValidationError::new(
                        "temporal.source_time",
                        "output frame is at or beyond the selected source interval end",
                    ));
                }
                Ok(time)
            }
            None => self.frame_rate.time_for_frame(frame),
        }
    }

    /// Returns `ceil(duration * fps)` using exact checked integer arithmetic.
    ///
    /// # Errors
    ///
    /// Returns `temporal.frame_count` when the exact product or rounded count
    /// exceeds the supported `u64` frame-count range.
    pub fn frames_for_duration(&self, duration: RationalTime) -> Result<u64, ValidationError> {
        let numerator = u128::from(duration.numerator) * u128::from(self.frame_rate.numerator);
        let denominator =
            u128::from(duration.denominator) * u128::from(self.frame_rate.denominator);
        let rounded = numerator.checked_add(denominator - 1).ok_or_else(|| {
            ValidationError::new(
                "temporal.frame_count",
                "exact duration-to-frame conversion overflowed",
            )
        })? / denominator;
        u64::try_from(rounded).map_err(|_| {
            ValidationError::new(
                "temporal.frame_count",
                "duration requires more than the supported frame count",
            )
        })
    }
}

/// The six domain-owned transition easing modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Hold,
    Linear,
    QuadraticIn,
    QuadraticOut,
    SmoothStep,
    SmoothInOut,
}

impl Easing {
    /// Maps clamped progress to an exact endpoint-preserving interpolation weight.
    pub fn weight(self, progress: f64) -> f64 {
        if !progress.is_finite() || progress <= 0.0 {
            return 0.0;
        }
        if progress >= 1.0 {
            return 1.0;
        }
        match self {
            Self::Hold => 0.0,
            Self::Linear => progress,
            Self::QuadraticIn => progress * progress,
            Self::QuadraticOut => 1.0 - (1.0 - progress) * (1.0 - progress),
            Self::SmoothStep => progress * progress * (3.0 - 2.0 * progress),
            Self::SmoothInOut if progress < 0.5 => 2.0 * progress * progress,
            Self::SmoothInOut => 1.0 - 2.0 * (1.0 - progress) * (1.0 - progress),
        }
    }
}

/// One authored scalar End value in the same authority space as its target.
///
/// Document fields store base values, channel pattern fields store additive
/// deltas, channel-output response fields store typed additive deltas, and
/// channel-specific fields store direct values. The current document remains
/// the complete Start authority.
#[derive(Clone, Debug, PartialEq)]
pub struct ScalarEndOverride {
    pub target: PropertyTarget,
    pub field: PropertyFieldId,
    pub end: f64,
    pub easing: Easing,
}

/// A grouped RGB End mode for one solid channel paint.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorEndMode {
    /// Interpolates canonical linear RGB and alpha directly to this endpoint.
    LinearColor { end: ColorValue },
    /// Rotates the current Start paint in encoded-sRGB HSL by an unwrapped angle.
    HueRotation { end_degrees: f64 },
}

/// One grouped solid-paint End override; channel opacity remains independent.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorEndOverride {
    pub channel_id: ChannelId,
    pub mode: ColorEndMode,
    pub easing: Easing,
}

/// One reusable End-frame override stored outside project-only timing.
#[derive(Clone, Debug, PartialEq)]
pub enum TemporalEndOverride {
    Scalar(ScalarEndOverride),
    Color(ColorEndOverride),
}

/// A displayed End-frame scalar edit submitted for domain-owned normalization.
#[derive(Clone, Debug, PartialEq)]
pub struct TemporalEndpointEdit {
    pub target: PropertyTarget,
    pub field: PropertyFieldId,
    /// The desired effective End value shown by a frontend.
    pub effective_end: f64,
    pub easing: Easing,
}

/// Retains one compatible channel or channel-output identity and its effective scalar value.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelScalarBatchValue {
    pub target: PropertyTarget,
    pub value: f64,
}

/// Projects an All-channel control without presenting differing values as one shared value.
/// The average is a derived editing reference; the individual values remain authoritative.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelScalarBatch {
    pub field: PropertyFieldId,
    pub values: Vec<ChannelScalarBatchValue>,
    pub average: f64,
    pub minimum: f64,
    pub maximum: f64,
}

/// Exact temporal authority used as a stale base and atomic command replacement.
#[derive(Clone, Debug, PartialEq)]
pub struct TemporalAuthority {
    pub project_timing: ProjectTiming,
    pub end_overrides: Vec<TemporalEndOverride>,
}

/// One atomic timing/End-override replacement owned by `DocumentHistory`.
#[derive(Clone, Debug, PartialEq)]
pub struct TemporalCommand {
    base: TemporalAuthority,
    start_document: Box<Document>,
    replacement: TemporalAuthority,
}

impl TemporalCommand {
    /// Returns the exact stale base captured when this command was built.
    pub fn base(&self) -> &TemporalAuthority {
        &self.base
    }

    /// Returns the complete replacement authority for persistence or review.
    pub fn replacement(&self) -> &TemporalAuthority {
        &self.replacement
    }
}

impl Document {
    /// Resolves all compatible channel-owned scalar targets through current descriptor authority.
    ///
    /// Shared output definitions never enter a batch. A materialized document projects its
    /// selected frame, while an ordinary document projects Start. Missing capabilities are
    /// omitted individually; no channel or painter-ordered output is substituted for another.
    ///
    /// # Errors
    /// Rejects fields with no compatible targets and nonfinite aggregate references.
    pub fn channel_scalar_batch(
        &self,
        field: PropertyFieldId,
    ) -> Result<ChannelScalarBatch, ValidationError> {
        let values = self
            .property_values()
            .into_iter()
            .filter(|value| {
                value.descriptor.field == field
                    && matches!(
                        value.descriptor.target,
                        PropertyTarget::Channel(_) | PropertyTarget::ChannelOutput(_, _)
                    )
                    && matches!(
                        value.descriptor.temporal,
                        TemporalCapability::Scalar | TemporalCapability::ScalarAndGroupedColor
                    )
                    && temporal_descriptor(self, value.descriptor.target, field).is_ok()
            })
            .filter_map(|value| match value.value {
                PropertyCurrentValueKind::FiniteF64(scalar) => Some(ChannelScalarBatchValue {
                    target: value.descriptor.target,
                    value: scalar,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        if values.is_empty() {
            return Err(ValidationError::new(
                "channel_batch.field",
                "no compatible channel values are available for this field",
            ));
        }
        let average = values
            .iter()
            .map(|value| value.value / values.len() as f64)
            .sum::<f64>();
        validate_finite(average, "channel_batch.average")?;
        let minimum = values
            .iter()
            .map(|value| value.value)
            .fold(f64::INFINITY, f64::min);
        let maximum = values
            .iter()
            .map(|value| value.value)
            .fold(f64::NEG_INFINITY, f64::max);
        Ok(ChannelScalarBatch {
            field,
            values,
            average,
            minimum,
            maximum,
        })
    }

    /// Moves compatible Start values to requested field averages while preserving their differences.
    ///
    /// Each tuple supplies a field and desired effective average. Coupled minimum/maximum fields
    /// may be supplied together and validate only after the complete unpublished batch exists.
    /// Other fields, source mappings, output definitions, End values and easing remain intact.
    /// The returned configuration publishes through the existing stale-aware history boundary.
    ///
    /// # Errors
    /// Rejects empty/duplicate fields, inactive targets, nonfinite arithmetic, invalid bounds or
    /// coupled responses without mutating Start, End, history, source or project timing.
    pub fn edit_all_channel_start_configuration(
        &self,
        edits: &[(PropertyFieldId, f64)],
    ) -> Result<DocumentConfiguration, ValidationError> {
        validate_channel_batch_fields(edits)?;
        let mut candidate = self.clone();
        for &(field, desired_average) in edits {
            let batch = self.channel_scalar_batch(field)?;
            let adjustment = desired_average - batch.average;
            validate_finite(adjustment, "channel_batch.adjustment")?;
            if adjustment == 0.0 {
                continue;
            }
            for value in batch.values {
                let authored = scalar_start(self, value.target, field)?;
                apply_scalar(&mut candidate, value.target, field, authored + adjustment)?;
            }
        }
        candidate.validate()?;
        Ok(DocumentConfiguration::capture(&candidate))
    }

    /// Moves compatible End values to requested field averages in one original-root command.
    ///
    /// Each channel/output retains its own offset and interpolation choice. Grouped color
    /// components delegate to color authoring, so alpha edits preserve each channel's distinct
    /// RGB or hue endpoint. Regular coupled fields are assembled before complete validation.
    /// Start, static output definitions and project timing remain unchanged.
    ///
    /// # Errors
    /// Rejects empty/duplicate fields, inapplicability, incompatible color ownership, invalid
    /// arithmetic, bounds or complete temporal responses without publishing a partial batch.
    pub fn edit_all_channel_end_command(
        &self,
        edits: &[(PropertyFieldId, f64)],
    ) -> Result<TemporalCommand, ValidationError> {
        validate_channel_batch_fields(edits)?;
        let end = materialize_progress(self, 1.0)?;
        let mut overrides = self.temporal_end_overrides.clone();
        let mut colors = Vec::new();
        for &(field, desired_average) in edits {
            let batch = end.channel_scalar_batch(field)?;
            let adjustment = desired_average - batch.average;
            validate_finite(adjustment, "channel_batch.adjustment")?;
            if adjustment == 0.0 {
                continue;
            }
            for value in batch.values {
                let effective_end = value.value + adjustment;
                let component = match field {
                    PropertyFieldId::ColorRed => Some(ColorComponent::Red),
                    PropertyFieldId::ColorGreen => Some(ColorComponent::Green),
                    PropertyFieldId::ColorBlue => Some(ColorComponent::Blue),
                    PropertyFieldId::ColorAlpha => Some(ColorComponent::Alpha),
                    _ => None,
                };
                if let (PropertyTarget::Channel(channel), Some(component)) =
                    (value.target, component)
                {
                    colors.push((channel, component, effective_end));
                    continue;
                }
                let easing = self
                    .temporal_end_overrides
                    .iter()
                    .find_map(|entry| match entry {
                        TemporalEndOverride::Scalar(current)
                            if current.target == value.target && current.field == field =>
                        {
                            Some(current.easing)
                        }
                        _ => None,
                    })
                    .unwrap_or(Easing::Linear);
                upsert_effective_scalar_end(
                    self,
                    &mut overrides,
                    &TemporalEndpointEdit {
                        target: value.target,
                        field,
                        effective_end,
                        easing,
                    },
                )?;
            }
        }
        let mut candidate = self.clone();
        candidate.temporal_end_overrides = overrides;
        for (channel, component, value) in colors {
            let command =
                candidate.edit_paint_component_end_command(&[channel], component, value)?;
            candidate.temporal_end_overrides = command.replacement.end_overrides;
        }
        candidate.validate()?;
        Ok(self.replace_temporal_authority_command(
            self.project_timing.clone(),
            candidate.temporal_end_overrides,
        ))
    }

    /// Reconstructs persisted project timing and reusable End overrides without history.
    ///
    /// # Errors
    ///
    /// Returns complete temporal or document validation diagnostics without modifying `self`.
    pub fn with_temporal_authority(
        mut self,
        project_timing: ProjectTiming,
        mut end_overrides: Vec<TemporalEndOverride>,
    ) -> Result<Self, ValidationError> {
        canonicalize_end_overrides(&mut end_overrides);
        self.project_timing = project_timing;
        self.temporal_end_overrides = end_overrides;
        self.validate()?;
        Ok(self)
    }

    /// Captures project timing and reusable End overrides as one exact stale base.
    pub fn temporal_authority(&self) -> TemporalAuthority {
        TemporalAuthority {
            project_timing: self.project_timing.clone(),
            end_overrides: self.temporal_end_overrides.clone(),
        }
    }

    /// Builds one complete atomic replacement command for timing and End overrides.
    pub fn replace_temporal_authority_command(
        &self,
        project_timing: ProjectTiming,
        mut end_overrides: Vec<TemporalEndOverride>,
    ) -> TemporalCommand {
        canonicalize_end_overrides(&mut end_overrides);
        TemporalCommand {
            base: self.temporal_authority(),
            start_document: Box::new(self.clone()),
            replacement: TemporalAuthority {
                project_timing,
                end_overrides,
            },
        }
    }

    /// Converts displayed effective End edits into authored base/delta overrides atomically.
    ///
    /// Document-base edits are installed first in the proposed override set.
    /// Named-channel inherited values are then converted against that End base,
    /// so omitted named overrides continue to inherit an animated document base.
    /// Equal Start/End intent retains an initialized End value. Multiple coupled
    /// fields may be supplied together and are validated only after the whole
    /// proposed End state is assembled.
    ///
    /// # Errors
    ///
    /// Returns target, capability, finite, coupled-range, stale-descriptor, or
    /// complete End-document diagnostics without mutating document or history.
    pub fn edit_effective_end_command(
        &self,
        edits: &[TemporalEndpointEdit],
    ) -> Result<TemporalCommand, ValidationError> {
        if edits.is_empty() {
            return Err(ValidationError::new(
                "temporal.end_edits",
                "at least one End-frame edit is required",
            ));
        }
        let mut edited_keys = HashSet::new();
        if edits
            .iter()
            .any(|edit| !edited_keys.insert((edit.target, edit.field)))
        {
            return Err(ValidationError::new(
                "temporal.end_edits.duplicate",
                "one atomic End edit may address each target and field only once",
            ));
        }
        let mut overrides = self.temporal_end_overrides.clone();
        for edit in edits
            .iter()
            .filter(|edit| edit.target == PropertyTarget::Document)
        {
            upsert_effective_scalar_end(self, &mut overrides, edit)?;
        }
        for edit in edits
            .iter()
            .filter(|edit| edit.target != PropertyTarget::Document)
        {
            upsert_effective_scalar_end(self, &mut overrides, edit)?;
        }
        let command =
            self.replace_temporal_authority_command(self.project_timing.clone(), overrides);
        command.validate(self)?;
        Ok(command)
    }

    /// Captures every still-uninitialized animatable End value from ordinary Start settings.
    ///
    /// Equal values are retained: their presence records initialization without duplicating
    /// Start or introducing a second document. Existing End values and easing stay untouched.
    /// Calling this again returns an equal replacement when all active fields are initialized.
    ///
    /// # Errors
    /// Rejects invalid descriptors, coupled End values, or a complete invalid candidate.
    pub fn initialize_end_command(&self) -> Result<TemporalCommand, ValidationError> {
        let mut overrides = self.temporal_end_overrides.clone();
        for descriptor in self.property_descriptors() {
            if !matches!(
                descriptor.temporal,
                TemporalCapability::Scalar | TemporalCapability::ScalarAndGroupedColor
            ) || temporal_descriptor(self, descriptor.target, descriptor.field).is_err()
            {
                continue;
            }
            let covered = overrides.iter().any(|entry| match entry {
                TemporalEndOverride::Scalar(value) => {
                    value.target == descriptor.target && value.field == descriptor.field
                }
                TemporalEndOverride::Color(value) => {
                    descriptor.target == PropertyTarget::Channel(value.channel_id)
                        && (matches!(
                            descriptor.field,
                            PropertyFieldId::ColorRed
                                | PropertyFieldId::ColorGreen
                                | PropertyFieldId::ColorBlue
                        ) || descriptor.field == PropertyFieldId::ColorAlpha
                            && matches!(value.mode, ColorEndMode::LinearColor { .. }))
                }
            });
            if covered {
                continue;
            }
            let mut end = scalar_start(self, descriptor.target, descriptor.field)?;
            if matches!(descriptor.target, PropertyTarget::Channel(_))
                && matches!(
                    descriptor.field,
                    PropertyFieldId::Density
                        | PropertyFieldId::DensityAspect
                        | PropertyFieldId::RotationDegrees
                        | PropertyFieldId::ShapeRotationDegrees
                )
            {
                end += scalar_start(self, PropertyTarget::Document, descriptor.field)?
                    - document_base_end(self, &overrides, descriptor.field)?;
            }
            overrides.push(TemporalEndOverride::Scalar(ScalarEndOverride {
                target: descriptor.target,
                field: descriptor.field,
                end,
                easing: Easing::Linear,
            }));
        }
        let command =
            self.replace_temporal_authority_command(self.project_timing.clone(), overrides);
        command.validate(self)?;
        Ok(command)
    }

    /// Adds, replaces, or removes one grouped color End mode as an atomic command.
    ///
    /// Passing `None` removes the channel's grouped color mode. Component RGB
    /// overrides remain mutually exclusive with either grouped mode, while a
    /// hue path may coexist with an independent scalar alpha override.
    ///
    /// # Errors
    ///
    /// Returns solid-paint, finite, component-conflict, or complete End-state diagnostics.
    pub fn edit_color_end_command(
        &self,
        channel_id: ChannelId,
        replacement: Option<(ColorEndMode, Easing)>,
    ) -> Result<TemporalCommand, ValidationError> {
        let mut overrides = self.temporal_end_overrides.clone();
        overrides.retain(|entry| {
            !matches!(entry, TemporalEndOverride::Color(value) if value.channel_id == channel_id)
        });
        if let Some((mode, easing)) = replacement {
            overrides.push(TemporalEndOverride::Color(ColorEndOverride {
                channel_id,
                mode,
                easing,
            }));
        }
        let command =
            self.replace_temporal_authority_command(self.project_timing.clone(), overrides);
        command.validate(self)?;
        Ok(command)
    }

    /// Materializes one immutable static frame through the existing effective resolver inputs.
    ///
    /// The source document, history, definition records, and stored End overrides
    /// remain unchanged. The returned document has no temporal overrides, so
    /// existing engine consumers can call `effective_channel_pattern` exactly once.
    ///
    /// # Errors
    ///
    /// Returns range, exact-time, target, finite, bounds, coupled-response, or
    /// complete document validation diagnostics for the requested frame.
    pub fn materialize_frame(&self, frame: u64) -> Result<Document, ValidationError> {
        let progress = self.project_timing.frame_range.progress(frame)?;
        materialize_progress(self, progress)
    }
}

/// Validates unique finite field-average requests before any unpublished batch mutation.
///
/// # Errors
/// Rejects empty batches, duplicate fields and nonfinite requested averages.
fn validate_channel_batch_fields(edits: &[(PropertyFieldId, f64)]) -> Result<(), ValidationError> {
    let mut seen = HashSet::new();
    if edits.is_empty() {
        return Err(ValidationError::new(
            "channel_batch.empty",
            "at least one field edit is required",
        ));
    }
    for &(field, value) in edits {
        if !seen.insert(field) {
            return Err(ValidationError::new(
                "channel_batch.duplicate",
                "each field may appear only once in a batch",
            ));
        }
        validate_finite(value, "channel_batch.value")?;
    }
    Ok(())
}

impl DocumentSession {
    /// Captures a materialized frame under the original document/revision token.
    ///
    /// This keeps scheduler stale-result checks tied to the authoritative
    /// session instead of minting a temporary revision-zero session.
    ///
    /// # Errors
    ///
    /// Returns the same frame materialization diagnostics as `Document::materialize_frame`.
    pub fn document_evaluation_snapshot_at_frame(
        &self,
        frame: u64,
    ) -> Result<DocumentEvaluationSnapshot, ValidationError> {
        Ok(DocumentEvaluationSnapshot {
            document: self.document.materialize_frame(frame)?,
            token: self.document_evaluation_token(),
        })
    }
}

impl DocumentHistory {
    /// Applies one temporal command as a single exact undoable history transition.
    ///
    /// # Errors
    ///
    /// Returns stale-base, validation, no-op, or revision-exhaustion diagnostics
    /// without changing the document or either history stack.
    pub fn apply_temporal(
        &mut self,
        command: &TemporalCommand,
    ) -> Result<CommandResult, DocumentSessionError> {
        command.validate(self.document())?;
        let before = self.session.snapshot();
        let candidate = command.apply(&before)?;
        if candidate == before {
            return Err(DocumentSessionError::Validation(ValidationError::new(
                "temporal.command",
                "temporal command is a semantic no-op",
            )));
        }
        let result = temporal_command_result(&before, &candidate);
        self.session.restore_history_snapshot(candidate.clone())?;
        self.undo.push(HistoryEntry {
            before,
            after: candidate,
            result: result.clone(),
        });
        self.redo.clear();
        Ok(result)
    }
}

impl TemporalCommand {
    /// Validates the exact stale base and complete replacement temporal authority.
    fn validate(&self, document: &Document) -> Result<(), ValidationError> {
        if document != self.start_document.as_ref() {
            return Err(ValidationError::new(
                "temporal.command.base",
                "temporal command base is stale",
            ));
        }
        let mut candidate = document.clone();
        candidate.project_timing = self.replacement.project_timing.clone();
        candidate.temporal_end_overrides = self.replacement.end_overrides.clone();
        candidate.validate()
    }

    /// Builds the validated unpublished replacement document.
    fn apply(&self, document: &Document) -> Result<Document, ValidationError> {
        self.validate(document)?;
        let mut candidate = document.clone();
        candidate.project_timing = self.replacement.project_timing.clone();
        candidate.temporal_end_overrides = self.replacement.end_overrides.clone();
        Ok(candidate)
    }
}

/// Validates normalized End overrides and their complete materialized End state.
///
/// # Errors
///
/// Returns deterministic target, duplicate, conflict, finite, bounds, or End-document diagnostics.
pub(crate) fn validate_temporal_authority(document: &Document) -> Result<(), ValidationError> {
    if document.temporal_end_overrides.len() > 16_384 {
        return Err(ValidationError::new(
            "temporal.end_overrides.limit",
            "documents support at most 16384 temporal End overrides",
        ));
    }
    let mut scalar_keys = HashSet::new();
    let mut color_channels = HashSet::new();
    for entry in &document.temporal_end_overrides {
        match entry {
            TemporalEndOverride::Scalar(value) => {
                validate_finite(value.end, "temporal.scalar.end")?;
                if !scalar_keys.insert((value.target, value.field)) {
                    return Err(ValidationError::new(
                        "temporal.scalar.duplicate",
                        "scalar End overrides require unique target and field pairs",
                    ));
                }
                let descriptor = temporal_descriptor(document, value.target, value.field)?;
                if !matches!(
                    descriptor.temporal,
                    TemporalCapability::Scalar | TemporalCapability::ScalarAndGroupedColor
                ) {
                    return Err(ValidationError::new(
                        "temporal.scalar.capability",
                        "property descriptor does not permit scalar animation",
                    ));
                }
            }
            TemporalEndOverride::Color(value) => {
                if !color_channels.insert(value.channel_id) {
                    return Err(ValidationError::new(
                        "temporal.color.duplicate",
                        "a channel may have only one grouped color End mode",
                    ));
                }
                let _ = solid_channel_color(document, value.channel_id)?;
                match &value.mode {
                    ColorEndMode::LinearColor { end } => validate_color_value(end)?,
                    ColorEndMode::HueRotation { end_degrees } => {
                        validate_finite(*end_degrees, "temporal.color.hue.end_degrees")?
                    }
                }
            }
        }
    }
    for channel_id in color_channels {
        let mode = document
            .temporal_end_overrides
            .iter()
            .find_map(|entry| match entry {
                TemporalEndOverride::Color(value) if value.channel_id == channel_id => {
                    Some(&value.mode)
                }
                _ => None,
            });
        for field in [
            PropertyFieldId::ColorRed,
            PropertyFieldId::ColorGreen,
            PropertyFieldId::ColorBlue,
        ] {
            if scalar_keys.contains(&(PropertyTarget::Channel(channel_id), field)) {
                return Err(ValidationError::new(
                    "temporal.color.conflict",
                    "grouped color and component RGB End overrides are mutually exclusive",
                ));
            }
        }
        if matches!(mode, Some(ColorEndMode::LinearColor { .. }))
            && scalar_keys.contains(&(
                PropertyTarget::Channel(channel_id),
                PropertyFieldId::ColorAlpha,
            ))
        {
            return Err(ValidationError::new(
                "temporal.color.conflict",
                "linear grouped color already owns the alpha End value",
            ));
        }
    }
    if document.temporal_end_overrides.is_empty() {
        return Ok(());
    }
    let _ = materialize_progress(document, 1.0)?;
    Ok(())
}

/// Resolves one active descriptor by stable field and target.
fn temporal_descriptor(
    document: &Document,
    target: PropertyTarget,
    field: PropertyFieldId,
) -> Result<PropertyDescriptor, ValidationError> {
    if field == PropertyFieldId::RotationDegrees {
        let scope = match target {
            PropertyTarget::Document => Some(PatternCapabilityScope::DocumentBase),
            PropertyTarget::Channel(channel) => Some(PatternCapabilityScope::Channel(channel)),
            _ => None,
        };
        if let Some(scope) = scope
            && !document
                .pattern_capabilities(scope)?
                .active_controls
                .iter()
                .any(|descriptor| descriptor.field == field)
        {
            return Err(ValidationError::new(
                "temporal.descriptor",
                "pattern rotation is inactive for this pattern",
            ));
        }
    }
    document
        .property_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.target == target && descriptor.field == field)
        .ok_or_else(|| {
            ValidationError::new(
                "temporal.descriptor",
                "temporal override targets an inactive or missing property descriptor",
            )
        })
}

/// Inserts a normalized effective End edit, retaining equal values as initialized intent.
fn upsert_effective_scalar_end(
    document: &Document,
    overrides: &mut Vec<TemporalEndOverride>,
    edit: &TemporalEndpointEdit,
) -> Result<(), ValidationError> {
    validate_finite(edit.effective_end, "temporal.end_edit.value")?;
    let descriptor = temporal_descriptor(document, edit.target, edit.field)?;
    if !matches!(
        descriptor.temporal,
        TemporalCapability::Scalar | TemporalCapability::ScalarAndGroupedColor
    ) {
        return Err(ValidationError::new(
            "temporal.end_edit.capability",
            "property descriptor does not permit an End-frame override",
        ));
    }
    let authored_end = normalize_effective_end(document, overrides, edit)?;
    let key_matches = |entry: &TemporalEndOverride| {
        matches!(
            entry,
            TemporalEndOverride::Scalar(value)
                if value.target == edit.target && value.field == edit.field
        )
    };
    let replacement = TemporalEndOverride::Scalar(ScalarEndOverride {
        target: edit.target,
        field: edit.field,
        end: authored_end,
        easing: edit.easing,
    });
    if let Some(index) = overrides.iter().position(key_matches) {
        overrides[index] = replacement;
    } else {
        overrides.push(replacement);
    }
    Ok(())
}

/// Converts a displayed effective End value to its persisted authority space.
fn normalize_effective_end(
    document: &Document,
    overrides: &[TemporalEndOverride],
    edit: &TemporalEndpointEdit,
) -> Result<f64, ValidationError> {
    match (edit.target, edit.field) {
        (
            PropertyTarget::Channel(_),
            PropertyFieldId::Density
            | PropertyFieldId::DensityAspect
            | PropertyFieldId::RotationDegrees
            | PropertyFieldId::ShapeRotationDegrees,
        ) => Ok(edit.effective_end - document_base_end(document, overrides, edit.field)?),
        (
            PropertyTarget::ChannelOutput(channel_id, output_layer_id),
            PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::ConnectedMinimumThickness
            | PropertyFieldId::ConnectedMaximumThickness
            | PropertyFieldId::CurveResponseBias
            | PropertyFieldId::RegionMinimumFill
            | PropertyFieldId::RegionMaximumFill,
        ) => Ok(edit.effective_end
            - output_base_scalar(document, channel_id, output_layer_id, edit.field)?),
        _ => Ok(edit.effective_end),
    }
}

/// Returns a document-base End value, including a proposed direct override.
fn document_base_end(
    document: &Document,
    overrides: &[TemporalEndOverride],
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    if let Some(end) = overrides.iter().find_map(|entry| match entry {
        TemporalEndOverride::Scalar(value)
            if value.target == PropertyTarget::Document && value.field == field =>
        {
            Some(value.end)
        }
        _ => None,
    }) {
        return Ok(end);
    }
    scalar_start(document, PropertyTarget::Document, field)
}

/// Returns one structural output's static scalar response authority.
fn output_base_scalar(
    document: &Document,
    channel_id: ChannelId,
    output_layer_id: PatternOutputLayerId,
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    let effective = document.effective_channel_pattern(channel_id)?;
    let response = document
        .bundle(effective.definition_id)
        .and_then(|bundle| {
            bundle
                .output_settings
                .iter()
                .find(|value| value.output_layer_id == output_layer_id)
        })
        .map(|value| &value.response)
        .ok_or_else(|| {
            ValidationError::new(
                "temporal.response.output",
                "response End override targets a missing structural output",
            )
        })?;
    response_scalar(response, field)
}

/// Returns one response component selected by an animatable response field.
fn response_scalar(
    response: &PatternGeometryResponse,
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    match (response, field) {
        (PatternGeometryResponse::Marks(value), PropertyFieldId::MarkMinimumFill) => {
            Ok(value.minimum_fill)
        }
        (PatternGeometryResponse::Marks(value), PropertyFieldId::MarkMaximumFill) => {
            Ok(value.maximum_fill)
        }
        (PatternGeometryResponse::Connected(value), PropertyFieldId::ConnectedMinimumThickness) => {
            Ok(value.minimum_thickness)
        }
        (PatternGeometryResponse::Connected(value), PropertyFieldId::ConnectedMaximumThickness) => {
            Ok(value.maximum_thickness)
        }
        (PatternGeometryResponse::Connected(value), PropertyFieldId::CurveResponseBias) => {
            Ok(value.bias)
        }
        (PatternGeometryResponse::Regions(value), PropertyFieldId::RegionMinimumFill) => {
            Ok(value.minimum_fill)
        }
        (PatternGeometryResponse::Regions(value), PropertyFieldId::RegionMaximumFill) => {
            Ok(value.maximum_fill)
        }
        _ => Err(ValidationError::new(
            "temporal.response.kind",
            "response End field does not match the selected output response kind",
        )),
    }
}

/// Reads a scalar Start value in its authored base, delta, or direct authority space.
fn scalar_start(
    document: &Document,
    target: PropertyTarget,
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    match (target, field) {
        (PropertyTarget::Document, PropertyFieldId::Density) => {
            Ok(document.pattern_settings.density.density)
        }
        (PropertyTarget::Document, PropertyFieldId::DensityAspect) => {
            Ok(document.pattern_settings.density.aspect)
        }
        (PropertyTarget::Document, PropertyFieldId::RotationDegrees) => {
            Ok(document.pattern_settings.pattern_rotation_degrees)
        }
        (PropertyTarget::Document, PropertyFieldId::ShapeRotationDegrees) => {
            Ok(document.pattern_settings.shape_rotation_degrees)
        }
        (PropertyTarget::Channel(channel_id), field) => {
            channel_scalar_start(document, channel_id, field)
        }
        (PropertyTarget::ChannelOutput(channel_id, output_layer_id), field) => {
            channel_output_scalar_start(document, channel_id, output_layer_id, field)
        }
        _ => Err(ValidationError::new(
            "temporal.scalar.target",
            "scalar End override uses an unsupported target and field combination",
        )),
    }
}

/// Reads a channel-owned scalar Start value without flattening inheritance.
fn channel_scalar_start(
    document: &Document,
    channel_id: ChannelId,
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    let instance = document
        .channel_pattern_instance(channel_id)
        .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))?;
    match field {
        PropertyFieldId::Density => Ok(instance
            .layout_delta
            .density
            .as_ref()
            .map_or(0.0, |value| value.density_delta)),
        PropertyFieldId::DensityAspect => Ok(instance
            .layout_delta
            .density
            .as_ref()
            .map_or(0.0, |value| value.aspect_delta)),
        PropertyFieldId::RotationDegrees => {
            Ok(instance.layout_delta.rotation_degrees.unwrap_or(0.0))
        }
        PropertyFieldId::ShapeRotationDegrees => {
            Ok(instance.shape_rotation_delta_degrees.unwrap_or(0.0))
        }
        PropertyFieldId::TranslationX => Ok(instance.layout_delta.translation_x),
        PropertyFieldId::TranslationY => Ok(instance.layout_delta.translation_y),
        PropertyFieldId::ModeledMappingGain => {
            modeled_channel(document, channel_id).map(|channel| channel.mapping.gain)
        }
        PropertyFieldId::ModeledMappingBias => {
            modeled_channel(document, channel_id).map(|channel| channel.mapping.bias)
        }
        PropertyFieldId::ColorRed => {
            solid_channel_color(document, channel_id).map(|value| value.red)
        }
        PropertyFieldId::ColorGreen => {
            solid_channel_color(document, channel_id).map(|value| value.green)
        }
        PropertyFieldId::ColorBlue => {
            solid_channel_color(document, channel_id).map(|value| value.blue)
        }
        PropertyFieldId::ColorAlpha => {
            solid_channel_color(document, channel_id).map(|value| value.alpha)
        }
        PropertyFieldId::Opacity => match &document.channel_configuration {
            ChannelConfiguration::Legacy(channels) => channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| channel.appearance.opacity)
                .ok_or_else(|| {
                    ValidationError::new("temporal.channel", "temporal channel is missing")
                }),
            ChannelConfiguration::Topology { topology, .. } => topology
                .channels
                .iter()
                .find(|channel| channel.id == channel_id)
                .map(|channel| channel.opacity)
                .ok_or_else(|| {
                    ValidationError::new("temporal.channel", "temporal channel is missing")
                }),
        },
        _ => Err(ValidationError::new(
            "temporal.scalar.channel_field",
            "field is not an animatable channel-owned scalar",
        )),
    }
}

/// Reads an optional channel-output delta component, using zero for inheritance.
fn channel_output_scalar_start(
    document: &Document,
    channel_id: ChannelId,
    output_layer_id: PatternOutputLayerId,
    field: PropertyFieldId,
) -> Result<f64, ValidationError> {
    let instance = document
        .channel_pattern_instance(channel_id)
        .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))?;
    let delta = instance
        .output_response_deltas
        .iter()
        .find(|value| value.output_layer_id == output_layer_id)
        .map(|value| &value.delta);
    match (delta, field) {
        (Some(ChannelGeometryResponseDelta::Marks(value)), PropertyFieldId::MarkMinimumFill) => {
            Ok(value.minimum_fill_delta.unwrap_or(0.0))
        }
        (Some(ChannelGeometryResponseDelta::Marks(value)), PropertyFieldId::MarkMaximumFill) => {
            Ok(value.maximum_fill_delta.unwrap_or(0.0))
        }
        (
            Some(ChannelGeometryResponseDelta::Connected(value)),
            PropertyFieldId::ConnectedMinimumThickness,
        ) => Ok(value.minimum_thickness_delta.unwrap_or(0.0)),
        (
            Some(ChannelGeometryResponseDelta::Connected(value)),
            PropertyFieldId::ConnectedMaximumThickness,
        ) => Ok(value.maximum_thickness_delta.unwrap_or(0.0)),
        (
            Some(ChannelGeometryResponseDelta::Connected(value)),
            PropertyFieldId::CurveResponseBias,
        ) => Ok(value.bias_delta.unwrap_or(0.0)),
        (
            Some(ChannelGeometryResponseDelta::Regions(value)),
            PropertyFieldId::RegionMinimumFill,
        ) => Ok(value.minimum_fill_delta.unwrap_or(0.0)),
        (
            Some(ChannelGeometryResponseDelta::Regions(value)),
            PropertyFieldId::RegionMaximumFill,
        ) => Ok(value.maximum_fill_delta.unwrap_or(0.0)),
        (None, field) if is_response_field(field) => {
            let _ = output_base_scalar(document, channel_id, output_layer_id, field)?;
            Ok(0.0)
        }
        _ => Err(ValidationError::new(
            "temporal.response.kind",
            "response End field does not match the channel output delta kind",
        )),
    }
}

/// Reports whether a scalar field belongs to channel-output response authority.
const fn is_response_field(field: PropertyFieldId) -> bool {
    matches!(
        field,
        PropertyFieldId::MarkMinimumFill
            | PropertyFieldId::MarkMaximumFill
            | PropertyFieldId::ConnectedMinimumThickness
            | PropertyFieldId::ConnectedMaximumThickness
            | PropertyFieldId::CurveResponseBias
            | PropertyFieldId::RegionMinimumFill
            | PropertyFieldId::RegionMaximumFill
    )
}

/// Preserves initialized End intent across Start edits and clears only invalidated dependencies.
///
/// Pattern replacement resets pattern-relative fields for its actual channels. Replacing the
/// shared document-base recipe is an All replacement, including document-level End fields. Translation,
/// paint, opacity and source mapping remain independent. Ordinary output-base edits rebase
/// stored End deltas, preserving effective End responses. Missing capabilities are pruned.
///
/// # Errors
/// Returns output-response authority diagnostics before the Start candidate is published.
pub(crate) fn reconcile_end_after_start(
    before: &Document,
    after: &mut Document,
    command: &DocumentCommand,
) -> Result<(), ValidationError> {
    if before.temporal_end_overrides.is_empty() {
        return Ok(());
    }
    let all = matches!(
        command,
        DocumentCommand::ReplaceDocumentPatternDefinitionRecipe { .. }
    ) || matches!(command, DocumentCommand::ReplaceSharedPatternDefinitionRecipe { definition_id, .. }
            if *definition_id == before.pattern_settings().definition_id)
        || matches!(command, DocumentCommand::SetDocumentPatternSettings { base, settings }
            if base.definition_id != settings.definition_id);
    let reset_channels = if all {
        after.channel_ids()
    } else {
        match command {
            DocumentCommand::SetChannelPatternDefinitionOverride { channel_id, .. }
            | DocumentCommand::ResetChannelPatternDefinitionOverride { channel_id, .. }
            | DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                channel_id, ..
            }
            | DocumentCommand::ReplaceSelectedChannelDefinitionTopology { channel_id, .. } => {
                vec![*channel_id]
            }
            DocumentCommand::ReplaceSharedPatternDefinitionRecipe { definition_id, .. } => {
                before.linked_channels(*definition_id)
            }
            _ => Vec::new(),
        }
    };
    let mut retained = Vec::new();
    for entry in &after.temporal_end_overrides {
        match entry {
            TemporalEndOverride::Color(value) => {
                if solid_channel_color(after, value.channel_id).is_ok() {
                    retained.push(entry.clone());
                }
            }
            TemporalEndOverride::Scalar(value) => {
                let pattern_field = matches!(
                    value.field,
                    PropertyFieldId::Density
                        | PropertyFieldId::DensityAspect
                        | PropertyFieldId::RotationDegrees
                        | PropertyFieldId::ShapeRotationDegrees
                ) || is_response_field(value.field);
                let reset = pattern_field
                    && match value.target {
                        PropertyTarget::Document => all,
                        PropertyTarget::Channel(channel)
                        | PropertyTarget::ChannelOutput(channel, _) => {
                            reset_channels.contains(&channel)
                        }
                        _ => false,
                    };
                if reset {
                    continue;
                }
                let mut value = value.clone();
                if let PropertyTarget::ChannelOutput(channel, output) = value.target {
                    let selected_copy = matches!(command,
                        DocumentCommand::EditSelectedChannelPatternDefinitionBundle { channel_id, .. }
                        | DocumentCommand::EditSelectedChannelPatternDefinition { channel_id, .. }
                        if *channel_id == channel);
                    let mut next_output = output;
                    if selected_copy {
                        let old_id = before.effective_channel_pattern(channel)?.definition_id;
                        let new_id = after.effective_channel_pattern(channel)?.definition_id;
                        if old_id != new_id {
                            let old = before.definition(old_id).ok_or_else(|| {
                                ValidationError::new("temporal.output", "missing source definition")
                            })?;
                            let new = after.definition(new_id).ok_or_else(|| {
                                ValidationError::new("temporal.output", "missing edited definition")
                            })?;
                            let Some(index) = old
                                .output_layers
                                .iter()
                                .position(|layer| layer.id() == output)
                            else {
                                continue;
                            };
                            let mut order: Vec<_> = (0..old.output_layers.len()).collect();
                            if let DocumentCommand::EditSelectedChannelPatternDefinitionBundle {
                                edit:
                                    PatternDefinitionBundleEdit::MoveOutputLayer {
                                        output_layer_id,
                                        painter_index,
                                    },
                                ..
                            } = command
                                && let Some(from) = old
                                    .output_layers
                                    .iter()
                                    .position(|layer| layer.id() == *output_layer_id)
                            {
                                let moved = order.remove(from);
                                order.insert(*painter_index, moved);
                            }
                            let Some(index) = order.iter().position(|original| *original == index)
                            else {
                                continue;
                            };
                            let Some(layer) = new.output_layers.get(index) else {
                                continue;
                            };
                            next_output = layer.id();
                        }
                    }
                    value.target = PropertyTarget::ChannelOutput(channel, next_output);
                    if temporal_descriptor(after, value.target, value.field).is_err() {
                        continue;
                    }
                    value.end += output_base_scalar(before, channel, output, value.field)?
                        - output_base_scalar(after, channel, next_output, value.field)?;
                }
                if temporal_descriptor(after, value.target, value.field).is_err() {
                    continue;
                }
                retained.push(TemporalEndOverride::Scalar(value));
            }
        }
    }
    canonicalize_end_overrides(&mut retained);
    after.temporal_end_overrides = retained;
    Ok(())
}

/// Materializes every override at one normalized progress value and validates the static result.
fn materialize_progress(document: &Document, progress: f64) -> Result<Document, ValidationError> {
    let mut candidate = document.clone();
    candidate.temporal_end_overrides.clear();
    if progress == 0.0 {
        candidate.validate()?;
        return Ok(candidate);
    }
    for entry in &document.temporal_end_overrides {
        if let TemporalEndOverride::Scalar(value) = entry {
            let start = scalar_start(document, value.target, value.field)?;
            let end = value.end;
            let weight = value.easing.weight(progress);
            if weight == 0.0 {
                continue;
            }
            apply_scalar(
                &mut candidate,
                value.target,
                value.field,
                lerp(start, end, weight),
            )?;
        }
    }
    for entry in &document.temporal_end_overrides {
        if let TemporalEndOverride::Color(value) = entry {
            if value.easing.weight(progress) == 0.0 {
                continue;
            }
            apply_color_override(document, &mut candidate, value, progress)?;
        }
    }
    candidate.validate()?;
    Ok(candidate)
}

/// Applies one interpolated scalar in its exact base, delta, or direct storage authority.
fn apply_scalar(
    document: &mut Document,
    target: PropertyTarget,
    field: PropertyFieldId,
    value: f64,
) -> Result<(), ValidationError> {
    validate_finite(value, "temporal.scalar.evaluated")?;
    match (target, field) {
        (PropertyTarget::Document, PropertyFieldId::Density) => {
            document.pattern_settings.density.density = value
        }
        (PropertyTarget::Document, PropertyFieldId::DensityAspect) => {
            document.pattern_settings.density.aspect = value
        }
        (PropertyTarget::Document, PropertyFieldId::RotationDegrees) => {
            document.pattern_settings.pattern_rotation_degrees = value
        }
        (PropertyTarget::Document, PropertyFieldId::ShapeRotationDegrees) => {
            document.pattern_settings.shape_rotation_degrees = value
        }
        (PropertyTarget::Channel(channel_id), field) => {
            apply_channel_scalar(document, channel_id, field, value)?
        }
        (PropertyTarget::ChannelOutput(channel_id, output_layer_id), field) => {
            apply_channel_output_scalar(document, channel_id, output_layer_id, field, value)?
        }
        _ => {
            return Err(ValidationError::new(
                "temporal.scalar.target",
                "scalar End override uses an unsupported target and field combination",
            ));
        }
    }
    Ok(())
}

/// Applies one interpolated direct or delta scalar to a named channel.
fn apply_channel_scalar(
    document: &mut Document,
    channel_id: ChannelId,
    field: PropertyFieldId,
    value: f64,
) -> Result<(), ValidationError> {
    match field {
        PropertyFieldId::Density | PropertyFieldId::DensityAspect => {
            let instance = document
                .channel_pattern_instance_mut(channel_id)
                .ok_or_else(|| {
                    ValidationError::new("temporal.channel", "temporal channel is missing")
                })?;
            let delta = instance
                .layout_delta
                .density
                .get_or_insert(DensityMetricDelta2D {
                    density_delta: 0.0,
                    aspect_delta: 0.0,
                });
            match field {
                PropertyFieldId::Density => delta.density_delta = value,
                PropertyFieldId::DensityAspect => delta.aspect_delta = value,
                _ => unreachable!(),
            }
        }
        PropertyFieldId::RotationDegrees => {
            document
                .channel_pattern_instance_mut(channel_id)
                .ok_or_else(|| {
                    ValidationError::new("temporal.channel", "temporal channel is missing")
                })?
                .layout_delta
                .rotation_degrees = Some(value);
        }
        PropertyFieldId::ShapeRotationDegrees => {
            document
                .channel_pattern_instance_mut(channel_id)
                .ok_or_else(|| {
                    ValidationError::new("temporal.channel", "temporal channel is missing")
                })?
                .shape_rotation_delta_degrees = Some(value);
        }
        PropertyFieldId::TranslationX => {
            document
                .channel_pattern_instance_mut(channel_id)
                .ok_or_else(|| {
                    ValidationError::new("temporal.channel", "temporal channel is missing")
                })?
                .layout_delta
                .translation_x = value;
        }
        PropertyFieldId::TranslationY => {
            document
                .channel_pattern_instance_mut(channel_id)
                .ok_or_else(|| {
                    ValidationError::new("temporal.channel", "temporal channel is missing")
                })?
                .layout_delta
                .translation_y = value;
        }
        PropertyFieldId::ModeledMappingGain | PropertyFieldId::ModeledMappingBias => {
            let channel = modeled_channel_mut(document, channel_id)?;
            match field {
                PropertyFieldId::ModeledMappingGain => channel.mapping.gain = value,
                PropertyFieldId::ModeledMappingBias => channel.mapping.bias = value,
                _ => unreachable!(),
            }
        }
        PropertyFieldId::ColorRed
        | PropertyFieldId::ColorGreen
        | PropertyFieldId::ColorBlue
        | PropertyFieldId::ColorAlpha => {
            let color = solid_channel_color_mut(document, channel_id)?;
            match field {
                PropertyFieldId::ColorRed => color.red = value,
                PropertyFieldId::ColorGreen => color.green = value,
                PropertyFieldId::ColorBlue => color.blue = value,
                PropertyFieldId::ColorAlpha => color.alpha = value,
                _ => unreachable!(),
            }
        }
        PropertyFieldId::Opacity => match &mut document.channel_configuration {
            ChannelConfiguration::Legacy(channels) => {
                channels
                    .iter_mut()
                    .find(|channel| channel.id == channel_id)
                    .ok_or_else(|| {
                        ValidationError::new("temporal.channel", "temporal channel is missing")
                    })?
                    .appearance
                    .opacity = value
            }
            ChannelConfiguration::Topology { topology, .. } => {
                topology
                    .channels
                    .iter_mut()
                    .find(|channel| channel.id == channel_id)
                    .ok_or_else(|| {
                        ValidationError::new("temporal.channel", "temporal channel is missing")
                    })?
                    .opacity = value
            }
        },
        _ => {
            return Err(ValidationError::new(
                "temporal.scalar.channel_field",
                "field is not an animatable channel-owned scalar",
            ));
        }
    }
    Ok(())
}

/// Applies one interpolated response delta without modifying shared output definitions.
fn apply_channel_output_scalar(
    document: &mut Document,
    channel_id: ChannelId,
    output_layer_id: PatternOutputLayerId,
    field: PropertyFieldId,
    value: f64,
) -> Result<(), ValidationError> {
    let definition_id = document
        .channel_pattern_instance(channel_id)
        .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))?
        .definition_override
        .unwrap_or(document.pattern_settings.definition_id);
    let bundle = document.bundle(definition_id).cloned().ok_or_else(|| {
        ValidationError::new("temporal.response.output", "response definition is missing")
    })?;
    let base_response = Some(&bundle)
        .and_then(|bundle| {
            bundle
                .output_settings
                .iter()
                .find(|setting| setting.output_layer_id == output_layer_id)
        })
        .map(|setting| setting.response.clone())
        .ok_or_else(|| {
            ValidationError::new(
                "temporal.response.output",
                "response End override targets a missing structural output",
            )
        })?;
    let instance = document
        .channel_pattern_instance_mut(channel_id)
        .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))?;
    let index = if let Some(index) = instance
        .output_response_deltas
        .iter()
        .position(|entry| entry.output_layer_id == output_layer_id)
    {
        index
    } else {
        let delta = match base_response {
            PatternGeometryResponse::Marks(_) => {
                ChannelGeometryResponseDelta::Marks(MarkGeometryResponseDelta {
                    minimum_fill_delta: None,
                    maximum_fill_delta: None,
                })
            }
            PatternGeometryResponse::Connected(_) => {
                ChannelGeometryResponseDelta::Connected(ConnectedGeometryResponseDelta {
                    minimum_thickness_delta: None,
                    maximum_thickness_delta: None,
                    bias_delta: None,
                })
            }
            PatternGeometryResponse::Regions(_) => {
                ChannelGeometryResponseDelta::Regions(RegionGeometryResponseDelta {
                    minimum_fill_delta: None,
                    maximum_fill_delta: None,
                })
            }
        };
        upsert_pattern_output_delta_in_order(
            &bundle,
            &mut instance.output_response_deltas,
            PatternOutputResponseDelta {
                output_layer_id,
                delta,
            },
        )?;
        instance
            .output_response_deltas
            .iter()
            .position(|entry| entry.output_layer_id == output_layer_id)
            .expect("inserted output delta exists")
    };
    match (&mut instance.output_response_deltas[index].delta, field) {
        (ChannelGeometryResponseDelta::Marks(delta), PropertyFieldId::MarkMinimumFill) => {
            delta.minimum_fill_delta = Some(value)
        }
        (ChannelGeometryResponseDelta::Marks(delta), PropertyFieldId::MarkMaximumFill) => {
            delta.maximum_fill_delta = Some(value)
        }
        (
            ChannelGeometryResponseDelta::Connected(delta),
            PropertyFieldId::ConnectedMinimumThickness,
        ) => delta.minimum_thickness_delta = Some(value),
        (
            ChannelGeometryResponseDelta::Connected(delta),
            PropertyFieldId::ConnectedMaximumThickness,
        ) => delta.maximum_thickness_delta = Some(value),
        (ChannelGeometryResponseDelta::Connected(delta), PropertyFieldId::CurveResponseBias) => {
            delta.bias_delta = Some(value)
        }
        (ChannelGeometryResponseDelta::Regions(delta), PropertyFieldId::RegionMinimumFill) => {
            delta.minimum_fill_delta = Some(value)
        }
        (ChannelGeometryResponseDelta::Regions(delta), PropertyFieldId::RegionMaximumFill) => {
            delta.maximum_fill_delta = Some(value)
        }
        _ => {
            return Err(ValidationError::new(
                "temporal.response.kind",
                "response End field does not match the channel output delta kind",
            ));
        }
    }
    Ok(())
}

/// Applies a grouped linear-color or HSL hue End override to one static frame candidate.
fn apply_color_override(
    start_document: &Document,
    candidate: &mut Document,
    value: &ColorEndOverride,
    progress: f64,
) -> Result<(), ValidationError> {
    let start = solid_channel_color(start_document, value.channel_id)?.clone();
    let weight = value.easing.weight(progress);
    match &value.mode {
        ColorEndMode::LinearColor { end } => {
            *solid_channel_color_mut(candidate, value.channel_id)? = ColorValue {
                red: lerp(start.red, end.red, weight),
                green: lerp(start.green, end.green, weight),
                blue: lerp(start.blue, end.blue, weight),
                alpha: lerp(start.alpha, end.alpha, weight),
            };
        }
        ColorEndMode::HueRotation { end_degrees } => {
            let degrees = lerp(0.0, *end_degrees, weight);
            let rotated = rotate_hue(&start, degrees);
            let candidate_color = solid_channel_color_mut(candidate, value.channel_id)?;
            candidate_color.red = rotated.red;
            candidate_color.green = rotated.green;
            candidate_color.blue = rotated.blue;
        }
    }
    Ok(())
}

/// Returns one modeled channel for mapping access.
fn modeled_channel(
    document: &Document,
    channel_id: ChannelId,
) -> Result<&ModeledChannelState, ValidationError> {
    let ChannelConfiguration::Topology { topology, .. } = &document.channel_configuration else {
        return Err(ValidationError::new(
            "temporal.mapping",
            "modeled mapping animation requires a modeled channel topology",
        ));
    };
    topology
        .channels
        .iter()
        .find(|channel| channel.id == channel_id)
        .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))
}

/// Returns one mutable modeled channel for materialization only.
fn modeled_channel_mut(
    document: &mut Document,
    channel_id: ChannelId,
) -> Result<&mut ModeledChannelState, ValidationError> {
    let ChannelConfiguration::Topology { topology, .. } = &mut document.channel_configuration
    else {
        return Err(ValidationError::new(
            "temporal.mapping",
            "modeled mapping animation requires a modeled channel topology",
        ));
    };
    topology
        .channels
        .iter_mut()
        .find(|channel| channel.id == channel_id)
        .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))
}

/// Returns one immutable solid channel paint across legacy and modeled storage.
pub(crate) fn solid_channel_color(
    document: &Document,
    channel_id: ChannelId,
) -> Result<&ColorValue, ValidationError> {
    match &document.channel_configuration {
        ChannelConfiguration::Legacy(channels) => channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .map(|channel| &channel.appearance.color)
            .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing")),
        ChannelConfiguration::Topology { topology, .. } => topology
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))
            .and_then(|channel| match &channel.paint {
                ChannelPaint::Solid(color) => Ok(color),
                ChannelPaint::SampledSource => Err(ValidationError::new(
                    "temporal.color.paint",
                    "grouped color animation requires solid channel paint",
                )),
            }),
    }
}

/// Returns one mutable solid paint for unpublished frame or authored-color candidates.
pub(crate) fn solid_channel_color_mut(
    document: &mut Document,
    channel_id: ChannelId,
) -> Result<&mut ColorValue, ValidationError> {
    match &mut document.channel_configuration {
        ChannelConfiguration::Legacy(channels) => channels
            .iter_mut()
            .find(|channel| channel.id == channel_id)
            .map(|channel| &mut channel.appearance.color)
            .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing")),
        ChannelConfiguration::Topology { topology, .. } => topology
            .channels
            .iter_mut()
            .find(|channel| channel.id == channel_id)
            .ok_or_else(|| ValidationError::new("temporal.channel", "temporal channel is missing"))
            .and_then(|channel| match &mut channel.paint {
                ChannelPaint::Solid(color) => Ok(color),
                ChannelPaint::SampledSource => Err(ValidationError::new(
                    "temporal.color.paint",
                    "grouped color animation requires solid channel paint",
                )),
            }),
    }
}

/// Validates one grouped canonical linear RGBA endpoint through unit-component bounds.
pub(crate) fn validate_color_value(color: &ColorValue) -> Result<(), ValidationError> {
    validate_unit_component(color.red, "temporal.color.end.red")?;
    validate_unit_component(color.green, "temporal.color.end.green")?;
    validate_unit_component(color.blue, "temporal.color.end.blue")?;
    validate_unit_component(color.alpha, "temporal.color.end.alpha")
}

/// Rotates one canonical linear paint through encoded-sRGB HSL without changing alpha.
fn rotate_hue(color: &ColorValue, degrees: f64) -> ColorValue {
    if degrees == 0.0 || degrees.rem_euclid(360.0) == 0.0 {
        return color.clone();
    }
    let encoded = [
        linear_to_srgb(color.red),
        linear_to_srgb(color.green),
        linear_to_srgb(color.blue),
    ];
    let (hue, saturation, lightness) = encoded_rgb_to_hsl(encoded);
    if saturation == 0.0 {
        return color.clone();
    }
    let rotated = hsl_to_encoded_rgb((hue + degrees).rem_euclid(360.0), saturation, lightness);
    ColorValue {
        red: srgb_to_linear(rotated[0]),
        green: srgb_to_linear(rotated[1]),
        blue: srgb_to_linear(rotated[2]),
        alpha: color.alpha,
    }
}

/// Converts a normalized linear-light component to encoded sRGB.
pub(crate) fn linear_to_srgb(value: f64) -> f64 {
    if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts a normalized encoded-sRGB component to linear light.
pub(crate) fn srgb_to_linear(value: f64) -> f64 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts normalized encoded sRGB to CSS-compatible HSL degrees/fractions.
fn encoded_rgb_to_hsl(rgb: [f64; 3]) -> (f64, f64, f64) {
    let maximum = rgb[0].max(rgb[1]).max(rgb[2]);
    let minimum = rgb[0].min(rgb[1]).min(rgb[2]);
    let delta = maximum - minimum;
    let lightness = (maximum + minimum) * 0.5;
    if delta == 0.0 {
        return (0.0, 0.0, lightness);
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue_sector = if maximum == rgb[0] {
        ((rgb[1] - rgb[2]) / delta).rem_euclid(6.0)
    } else if maximum == rgb[1] {
        (rgb[2] - rgb[0]) / delta + 2.0
    } else {
        (rgb[0] - rgb[1]) / delta + 4.0
    };
    (hue_sector * 60.0, saturation, lightness)
}

/// Converts CSS-compatible HSL degrees/fractions to normalized encoded sRGB.
fn hsl_to_encoded_rgb(hue: f64, saturation: f64, lightness: f64) -> [f64; 3] {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let x = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let rgb = match sector.floor() as i32 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    let offset = lightness - chroma * 0.5;
    [rgb[0] + offset, rgb[1] + offset, rgb[2] + offset]
}

/// Interpolates with exact endpoint selection to preserve authored values bit-for-bit.
fn lerp(start: f64, end: f64, weight: f64) -> f64 {
    if weight <= 0.0 {
        start
    } else if weight >= 1.0 {
        end
    } else {
        if start.is_sign_negative() != end.is_sign_negative() {
            start * (1.0 - weight) + end * weight
        } else {
            start + (end - start) * weight
        }
    }
}

/// Returns the strongest cache invalidation implied by a temporal authority change.
pub(crate) fn temporal_command_result(before: &Document, after: &Document) -> CommandResult {
    let mut affected = Vec::new();
    let mut invalidation = None;
    if before.project_timing != after.project_timing {
        affected = before.channel_ids();
        // Timing can select different source pixels; narrower per-frame caches still compare content.
        invalidation = Some(InvalidationLevel::Source);
    }
    for entry in before
        .temporal_end_overrides
        .iter()
        .chain(&after.temporal_end_overrides)
    {
        match entry {
            TemporalEndOverride::Scalar(value) => {
                invalidation = strongest_invalidation(
                    invalidation,
                    property_field_contract(value.field).invalidation,
                );
                match value.target {
                    PropertyTarget::Document => {
                        for channel_id in before.channel_ids() {
                            if !affected.contains(&channel_id) {
                                affected.push(channel_id);
                            }
                        }
                    }
                    PropertyTarget::Channel(channel_id)
                    | PropertyTarget::ChannelOutput(channel_id, _) => {
                        if !affected.contains(&channel_id) {
                            affected.push(channel_id);
                        }
                    }
                    _ => {}
                }
            }
            TemporalEndOverride::Color(value) => {
                invalidation =
                    strongest_invalidation(invalidation, InvalidationLevel::Presentation);
                if !affected.contains(&value.channel_id) {
                    affected.push(value.channel_id);
                }
            }
        }
    }
    let order = before.channel_ids();
    affected.sort_by_key(|channel_id| {
        order
            .iter()
            .position(|candidate| candidate == channel_id)
            .unwrap_or(usize::MAX)
    });
    CommandResult {
        affected_channels: affected,
        invalidation,
        created_authored_structure_id: None,
    }
}

/// Orders End overrides by stable target/field identity before storage and no-op comparison.
fn canonicalize_end_overrides(overrides: &mut [TemporalEndOverride]) {
    overrides.sort_by_key(|entry| match entry {
        TemporalEndOverride::Scalar(value) => {
            let (scope, channel, output) = match value.target {
                PropertyTarget::Document => (0, 0, 0),
                PropertyTarget::Channel(channel) => (1, channel.0, 0),
                PropertyTarget::ChannelOutput(channel, output) => (2, channel.0, output.0),
                _ => (3, 0, 0),
            };
            (scope, channel, output, value.field as u16)
        }
        TemporalEndOverride::Color(value) => (1, value.channel_id.0, 0, u16::MAX),
    });
}

/// Computes the greatest common divisor for exact rational reduction.
const fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    if left == 0 { 1 } else { left }
}

/// Computes the greatest common divisor for intermediate exact-time products.
const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    if left == 0 { 1 } else { left }
}
