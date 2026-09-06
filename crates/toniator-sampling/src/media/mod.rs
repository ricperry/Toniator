//! Bounded still and moving-media frame providers.

mod animated_image;
mod frame_source;
mod image_sequence;
mod still;
mod video;

pub use animated_image::{AnimatedImageFormat, AnimatedImageSource, is_animated_image};
pub use frame_source::{
    FrameIdentity, FrameSource, MediaTools, SourceError, SourceFrame, SourceMedia, SourceMediaKind,
    SourceMediaMetadata,
};
pub use image_sequence::{ImageSequenceEntry, ImageSequenceSource};
pub use still::StillImageSource;
pub use video::VideoSource;
