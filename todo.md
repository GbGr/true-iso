Create a Rust console utility that corrects isometric 2D tile sprites to mathematically consistent proportions using geometric transforms (e.g., skew/affine/perspective), not by simply masking/cropping. The tool takes an input image path and an optional output path. If the output path is not provided, save the result next to the input file using the same name with the suffix `_corrected` (e.g., `tile.png` → `tile_corrected.png`). Assume each input image contains exactly one sprite/tile on a transparent background (prefer PNG with alpha). The tool should preserve transparency and avoid introducing opaque borders.

Provide a clean CLI with helpful `--help` output, basic validation, and clear error messages. Include a configurable option to choose the target isometric proportion (e.g., a default “2:1 isometric” ratio, with the ability to override).

IT SHOULD BE SUPER SMART! IT SHOULD READ SOURCE IMAGE AND FULLY UNDERSTAND THE PROBLEM. IMAGES MAY HAVE PADDING AND BLOCKS CAN HAVE INCORRECT ISOMETRY. GENERATOR SHOULD COVER THESE CASES!

Project structure requirements:

* Use Cargo for the Rust project.
* Add an `examples/` folder in the repository containing several source images that need correction (these are used for manual testing and regression checks).
* Document in `README.md` how to run the tool against images in `examples/`, including example commands and the default output naming behavior.

Deliverables:

* Working Rust source code for the CLI tool.
* `Cargo.toml` and any required project files.
* `README.md` with usage instructions and example invocations using the `examples/` images.
