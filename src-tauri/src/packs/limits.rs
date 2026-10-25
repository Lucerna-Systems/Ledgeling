//! Hard ceilings on what a pack may contain. A pack is untrusted input the
//! moment it comes from anywhere but the author's own disk, so everything that
//! allocates in proportion to pack content is capped here.

/// Archive (`.petpack`) limits, enforced while extracting.
pub const MAX_ARCHIVE_ENTRIES: usize = 64;
pub const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_UNCOMPRESSED_TOTAL: u64 = 16 * 1024 * 1024;
pub const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024;

/// The manifest is text; nothing legitimate comes close to this.
pub const MAX_MANIFEST_BYTES: u64 = 256 * 1024;

/// Image limits, checked from the PNG header before any pixel is decoded.
pub const MAX_PNG_WIDTH: u32 = 4096;
pub const MAX_PNG_HEIGHT: u32 = 1024;

/// Animation limits.
pub const MAX_FRAMES: u32 = 64;
pub const MAX_FPS: f64 = 60.;
pub const MAX_ANIMATIONS: usize = 32;

/// Behaviour limits: a pack can be valid and still expensive to run.
pub const MAX_STATES: usize = 64;
pub const MAX_TRANSITIONS_PER_STATE: usize = 16;
pub const MAX_REACTIONS: usize = 64;

/// Text limits (characters).
pub const MAX_NAME_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 2000;
pub const MAX_LICENSE_CHARS: usize = 64;
pub const MAX_TAGS: usize = 10;
pub const MAX_TAG_CHARS: usize = 40;

/// Publishing is stricter than loading.
pub const PUBLISH_SPRITE_SIZE: u32 = 48;
pub const PREVIEW_SIZE: u32 = 256;
