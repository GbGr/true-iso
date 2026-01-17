use anyhow::{Context, Result};
use clap::Parser;
use image::ImageReader;

use true_iso::{
    apply_affine_transform, compute_correction_matrix, crop_to_content, detect_isometric_angles,
    resize_to_fit, Cli,
};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load input image
    let img = ImageReader::open(&cli.input)
        .with_context(|| format!("Failed to open input file: {:?}", cli.input))?
        .decode()
        .with_context(|| format!("Failed to decode image: {:?}", cli.input))?;

    if cli.verbose {
        let (width, height) = (img.width(), img.height());
        eprintln!("Loaded image: {:?} ({}x{})", cli.input, width, height);
        eprintln!("Target ratio: {}:{}", cli.ratio.horizontal, cli.ratio.vertical);
        eprintln!("Target angle: {:.3}°", cli.ratio.target_angle_degrees());
        eprintln!();
    }

    // Detect isometric angles
    let geometry = detect_isometric_angles(&img, cli.verbose)
        .context("Failed to detect isometric geometry")?;

    if cli.verbose {
        eprintln!();
    }

    // Check if correction is needed
    let tolerance = 2.0; // degrees
    if geometry.angles.is_close_to_target(&cli.ratio, tolerance) {
        eprintln!(
            "Image already has correct isometric proportions (within {:.1}° tolerance)",
            tolerance
        );
        if cli.verbose {
            eprintln!(
                "Detected: left={:.2}°, right={:.2}°",
                geometry.angles.left_angle, geometry.angles.right_angle
            );
            eprintln!("Target: ±{:.3}°", cli.ratio.target_angle_degrees());
        }
        // Still crop and resize even if angles are correct
        let output_path = cli.output_path();
        let rgba = img.to_rgba8();
        let cropped = crop_to_content(&rgba);
        let final_image = resize_to_fit(&cropped, cli.size);

        final_image
            .save(&output_path)
            .with_context(|| format!("Failed to save output: {:?}", output_path))?;
        eprintln!(
            "Saved (angles unchanged, cropped & resized): {:?}",
            output_path
        );
        eprintln!(
            "Dimensions: {}x{} -> {}x{}",
            img.width(),
            img.height(),
            final_image.width(),
            final_image.height()
        );
        return Ok(());
    }

    // Report detected angles
    eprintln!(
        "Detected angles: left={:.2}°, right={:.2}°",
        geometry.angles.left_angle, geometry.angles.right_angle
    );
    eprintln!(
        "Target angles: left=-{:.3}°, right=+{:.3}°",
        cli.ratio.target_angle_degrees(),
        cli.ratio.target_angle_degrees()
    );

    // Compute correction matrix
    let correction_matrix = compute_correction_matrix(
        &geometry.angles,
        &cli.ratio,
        geometry.center,
    );

    if cli.verbose {
        eprintln!();
        eprintln!("Correction matrix:");
        for row in 0..3 {
            eprintln!(
                "  [{:8.4}, {:8.4}, {:8.4}]",
                correction_matrix[(row, 0)],
                correction_matrix[(row, 1)],
                correction_matrix[(row, 2)]
            );
        }
        eprintln!();
    }

    // Apply transformation
    let rgba = img.to_rgba8();
    let transformed = apply_affine_transform(&rgba, &correction_matrix, cli.verbose);

    // Crop to content (remove padding)
    let cropped = crop_to_content(&transformed);

    if cli.verbose {
        eprintln!(
            "Cropped: {}x{} -> {}x{}",
            transformed.width(),
            transformed.height(),
            cropped.width(),
            cropped.height()
        );
    }

    // Resize to target size
    let final_image = resize_to_fit(&cropped, cli.size);

    if cli.verbose {
        eprintln!(
            "Resized: {}x{} -> {}x{} (target: {})",
            cropped.width(),
            cropped.height(),
            final_image.width(),
            final_image.height(),
            cli.size
        );
    }

    // Save result
    let output_path = cli.output_path();
    final_image
        .save(&output_path)
        .with_context(|| format!("Failed to save output: {:?}", output_path))?;

    eprintln!();
    eprintln!("Saved corrected image: {:?}", output_path);
    eprintln!(
        "Dimensions: {}x{} -> {}x{}",
        img.width(),
        img.height(),
        final_image.width(),
        final_image.height()
    );

    Ok(())
}
