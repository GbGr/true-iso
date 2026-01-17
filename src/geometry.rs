use nalgebra::{Matrix3, Vector2};

/// Represents an isometric projection ratio (horizontal:vertical)
/// For standard 2:1 isometric, this means 2 pixels horizontal per 1 pixel vertical
#[derive(Debug, Clone, Copy)]
pub struct IsometricRatio {
    pub horizontal: f64,
    pub vertical: f64,
}

impl IsometricRatio {
    pub fn new(horizontal: f64, vertical: f64) -> Self {
        Self { horizontal, vertical }
    }

    /// Returns the target angle in radians for this ratio
    /// For 2:1, this is arctan(0.5) ≈ 26.565°
    pub fn target_angle(&self) -> f64 {
        (self.vertical / self.horizontal).atan()
    }

    /// Returns the target angle in degrees
    pub fn target_angle_degrees(&self) -> f64 {
        self.target_angle().to_degrees()
    }
}

impl Default for IsometricRatio {
    fn default() -> Self {
        Self::new(2.0, 1.0)
    }
}

/// Detected angles from the isometric sprite
#[derive(Debug, Clone)]
pub struct DetectedAngles {
    /// Left-sloping angle (negative, typically around -26.565° for correct iso)
    pub left_angle: f64,
    /// Right-sloping angle (positive, typically around +26.565° for correct iso)
    pub right_angle: f64,
    /// Confidence in the left angle detection (0.0 to 1.0)
    pub left_confidence: f64,
    /// Confidence in the right angle detection (0.0 to 1.0)
    pub right_confidence: f64,
}

impl DetectedAngles {
    pub fn new(left_angle: f64, right_angle: f64, left_confidence: f64, right_confidence: f64) -> Self {
        Self {
            left_angle,
            right_angle,
            left_confidence,
            right_confidence,
        }
    }

    /// Check if the detected angles are close to the target
    pub fn is_close_to_target(&self, target: &IsometricRatio, tolerance_degrees: f64) -> bool {
        let target_angle = target.target_angle_degrees();
        let left_diff = (self.left_angle.abs() - target_angle).abs();
        let right_diff = (self.right_angle - target_angle).abs();
        left_diff < tolerance_degrees && right_diff < tolerance_degrees
    }
}

/// Compute the affine correction matrix to transform from detected angles to target angles
///
/// The transform is computed as: M = B_target × B_current⁻¹
/// where B represents the basis formed by the isometric axes
pub fn compute_correction_matrix(
    detected: &DetectedAngles,
    target: &IsometricRatio,
    center: (f64, f64),
) -> Matrix3<f64> {
    let target_angle = target.target_angle();

    // Current basis vectors (from detected angles)
    let left_rad = detected.left_angle.to_radians();
    let right_rad = detected.right_angle.to_radians();

    // Unit vectors along the detected isometric axes
    let current_left = Vector2::new(left_rad.cos(), left_rad.sin());
    let current_right = Vector2::new(right_rad.cos(), right_rad.sin());

    // Target basis vectors (for perfect 2:1 isometric)
    // Left axis goes up-left (negative angle), right axis goes up-right (positive angle)
    let target_left = Vector2::new((-target_angle).cos(), (-target_angle).sin());
    let target_right = Vector2::new(target_angle.cos(), target_angle.sin());

    // Build 2x2 basis matrices
    // B_current maps from iso-space to image-space
    let b_current = nalgebra::Matrix2::from_columns(&[current_left, current_right]);
    let b_target = nalgebra::Matrix2::from_columns(&[target_left, target_right]);

    // Compute the transformation: M = B_target × B_current⁻¹
    let transform_2x2 = match b_current.try_inverse() {
        Some(inv) => b_target * inv,
        None => nalgebra::Matrix2::identity(), // Fallback if singular
    };

    // Build full 3x3 affine matrix with translation to center
    let (cx, cy) = center;

    // Translate to origin, apply transform, translate back
    let translate_to_origin = Matrix3::new(
        1.0, 0.0, -cx,
        0.0, 1.0, -cy,
        0.0, 0.0, 1.0,
    );

    let transform = Matrix3::new(
        transform_2x2[(0, 0)], transform_2x2[(0, 1)], 0.0,
        transform_2x2[(1, 0)], transform_2x2[(1, 1)], 0.0,
        0.0, 0.0, 1.0,
    );

    let translate_back = Matrix3::new(
        1.0, 0.0, cx,
        0.0, 1.0, cy,
        0.0, 0.0, 1.0,
    );

    translate_back * transform * translate_to_origin
}

/// Transform a point using the affine matrix
pub fn transform_point(matrix: &Matrix3<f64>, x: f64, y: f64) -> (f64, f64) {
    let p = nalgebra::Vector3::new(x, y, 1.0);
    let result = matrix * p;
    (result.x / result.z, result.y / result.z)
}

/// Compute the bounding box of the transformed image
pub fn compute_output_bounds(
    matrix: &Matrix3<f64>,
    width: u32,
    height: u32,
) -> (u32, u32, f64, f64) {
    let corners = [
        (0.0, 0.0),
        (width as f64, 0.0),
        (0.0, height as f64),
        (width as f64, height as f64),
    ];

    let transformed: Vec<(f64, f64)> = corners
        .iter()
        .map(|&(x, y)| transform_point(matrix, x, y))
        .collect();

    let min_x = transformed.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = transformed.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = transformed.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = transformed.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    let new_width = (max_x - min_x).ceil() as u32;
    let new_height = (max_y - min_y).ceil() as u32;

    (new_width, new_height, min_x, min_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isometric_ratio_angle() {
        let ratio = IsometricRatio::new(2.0, 1.0);
        let angle = ratio.target_angle_degrees();
        assert!((angle - 26.565).abs() < 0.01);
    }

    #[test]
    fn test_identity_transform() {
        let detected = DetectedAngles::new(-26.565, 26.565, 1.0, 1.0);
        let target = IsometricRatio::new(2.0, 1.0);
        let matrix = compute_correction_matrix(&detected, &target, (50.0, 50.0));

        // Should be close to identity since detected ≈ target
        let (x, y) = transform_point(&matrix, 50.0, 50.0);
        assert!((x - 50.0).abs() < 0.1);
        assert!((y - 50.0).abs() < 0.1);
    }
}
