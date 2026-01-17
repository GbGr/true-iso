# true-iso

A CLI tool that corrects isometric tile sprites to mathematically consistent 2:1 proportions using geometric affine transformations.

Unlike simple cropping or masking approaches, **true-iso** automatically detects the current isometric angles in your sprite and applies precise mathematical transformations to correct them to a target isometric ratio.

## Features

- **Automatic angle detection** — Uses Canny edge detection and Hough transforms to identify sprite geometry
- **Geometric correction** — Applies affine transformations to fix isometric proportions
- **Smart padding handling** — Automatically removes transparent padding and crops to content
- **High-quality output** — Bicubic interpolation with proper alpha handling prevents artifacts
- **Configurable ratio** — Supports any isometric ratio (default: 2:1)
- **Tolerance checking** — Skips transformation if sprite is already within 2° of target

## Installation

### From source

```bash
git clone https://github.com/yourusername/true-iso.git
cd true-iso
cargo build --release
```

The binary will be available at `./target/release/true-iso`.

### Requirements

- Rust 1.70 or later

## Usage

### Basic usage

```bash
true-iso input.png
```

This will:
1. Detect the isometric angles in `input.png`
2. Correct them to the standard 2:1 ratio (26.565°)
3. Resize to 256px (longest side)
4. Save as `input_corrected.png`

### Specify output path

```bash
true-iso input.png -o output.png
```

### Custom isometric ratio

```bash
# 3:1 isometric ratio
true-iso input.png --ratio 3:1

# 4:1 isometric ratio
true-iso input.png -r 4:1
```

### Custom output size

```bash
# 512px output (longest side)
true-iso input.png --size 512

# 128px output
true-iso input.png -s 128
```

### Verbose mode

```bash
true-iso input.png --verbose
```

Shows detection details including:
- Detected left/right angles
- Target angles for the specified ratio
- Whether transformation was applied or skipped

### Combined options

```bash
true-iso sprite.png -o corrected.png --ratio 2:1 --size 512 --verbose
```

## Options Reference

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `<INPUT>` | — | *required* | Input PNG image path |
| `--output` | `-o` | `<input>_corrected.png` | Output file path |
| `--ratio` | `-r` | `2:1` | Target isometric ratio (`H:V`) |
| `--size` | `-s` | `256` | Output size in pixels (longest side) |
| `--verbose` | — | `false` | Show detection and transformation details |

## How It Works

```
Input PNG
    ↓
Load and detect sprite bounds
    ↓
Edge detection (Canny algorithm)
    ↓
Line detection (Hough transform)
    ↓
Angle classification (left/right slopes)
    ↓
Compute correction matrix
    ↓
Apply affine transformation
    ↓
Crop to content & resize
    ↓
Output PNG
```

The tool identifies the isometric angles in your sprite by analyzing edge lines, then computes an affine transformation matrix that maps the current angles to the target ratio. The transformation uses inverse mapping with bicubic interpolation for high-quality results.

## Examples

The `examples/` directory contains sample sprites for testing:

```bash
# Process all examples
for f in examples/*.png; do
  true-iso "$f" --verbose
done
```

---

## For Developers

### Project Structure

```
true-iso/
├── Cargo.toml          # Dependencies and build config
├── src/
│   ├── main.rs         # CLI entry point
│   ├── lib.rs          # Public API exports
│   ├── cli.rs          # Argument parsing (clap)
│   ├── detection.rs    # Angle detection pipeline
│   ├── geometry.rs     # Transformation math
│   └── transform.rs    # Image transformation
└── examples/           # Test images
```

### Module Overview

- **cli** — Command-line interface using `clap` derive macros
- **detection** — Sprite bounds detection, Canny edge detection, Hough line detection, angle classification
- **geometry** — Isometric ratio math, affine transformation matrices, coordinate mapping
- **transform** — Image interpolation (bicubic/bilinear), alpha handling, cropping, resizing

### Dependencies

| Crate | Purpose |
|-------|---------|
| `image` | Image I/O and basic operations |
| `imageproc` | Canny edge detection, Hough transform |
| `clap` | CLI argument parsing |
| `nalgebra` | Linear algebra (matrices, vectors) |
| `anyhow` | Error handling |

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test
```

### Key Algorithms

**Angle Detection:**
1. Find sprite bounds (non-transparent pixels)
2. Convert to grayscale with alpha masking
3. Apply Canny edge detection
4. Run Hough line transform
5. Classify lines into left-sloping (−60° to −15°) and right-sloping (15° to 60°)
6. Compute weighted median of angles (weighted by line length)

**Transformation:**
1. Build basis vectors from detected angles
2. Build target basis vectors from desired ratio
3. Compute affine matrix: `M = B_target × B_current⁻¹`
4. Apply inverse mapping with bicubic interpolation
5. Pre-multiply alpha before interpolation, unpremultiply after

### Mathematical Notes

For a 2:1 isometric ratio:
- Target angle = arctan(1/2) ≈ 26.565°
- Left axis: −26.565°
- Right axis: +26.565°

The affine transformation preserves the sprite's visual appearance while correcting the geometric proportions.

## License

MIT
