pub mod cli;
pub mod detection;
pub mod geometry;
pub mod transform;

pub use cli::Cli;
pub use detection::{detect_isometric_angles, DetectedGeometry};
pub use geometry::{compute_correction_matrix, IsometricRatio};
pub use transform::{apply_affine_transform, crop_to_content, resize_to_fit};
