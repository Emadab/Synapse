//! # synapse-media
//!
//! Media lives on disk under the collection folder (`collection.media/`); this
//! crate maintains the metadata index used for deduplication (by checksum),
//! unused-media cleanup and import/export reference rewriting. Implements
//! [`synapse_core::ports::MediaStore`].
//!
//! Real implementation lands alongside import (M2).
