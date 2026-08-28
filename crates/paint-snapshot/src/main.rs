use std::path::Path;
use std::process::ExitCode;

use paint_snapshot::{comparison, difference, render, Snapshot};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["render", snapshot, output] => {
            let snapshot = read(snapshot)?;
            let image = render(&snapshot)?;
            write(output, &image)?;
            println!("wrote {output} ({}x{})", image.width(), image.height());
            println!("{}", describe(&snapshot));
            Ok(())
        }
        ["diff", before, after, output] => {
            let (before, after) = (read(before)?, read(after)?);
            if let Some(difference) = difference(&before, &after) {
                println!("{}", difference.description);
            } else {
                println!("the snapshots are identical");
            }
            let (image, differing) = comparison(&before, &after)?;
            write(output, &image)?;
            println!("wrote {output}, {differing} pixels differ");
            println!("panels are, left to right: before, after, changed pixels");
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn describe(snapshot: &Snapshot) -> String {
    let textures: usize = snapshot
        .textures
        .values()
        .map(|texture| texture.png.len())
        .sum();
    let sizes: Vec<String> = snapshot
        .textures
        .values()
        .map(|texture| format!("{}x{}", texture.size[0], texture.size[1]))
        .collect();
    format!(
        "{} draw calls, {} textures ({}) taking {} bytes",
        snapshot.primitives.len(),
        snapshot.textures.len(),
        sizes.join(", "),
        textures
    )
}

fn usage() -> String {
    [
        "Renders paint snapshots so a person can look at them.",
        "",
        "Usage:",
        "  paint-snapshot render <snapshot> <output.png>",
        "  paint-snapshot diff <before> <after> <output.png>",
    ]
    .join("\n")
}

fn read(path: &str) -> Result<Snapshot, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("could not read {path}: {error}"))?;
    Snapshot::decode(&bytes).map_err(|error| format!("{path}: {error}"))
}

fn write(path: &str, image: &image::RgbaImage) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
    }
    image
        .save(path)
        .map_err(|error| format!("could not write {path}: {error}"))
}
