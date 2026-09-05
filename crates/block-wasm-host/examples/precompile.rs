use std::path::{Path, PathBuf};

use block_wasm_host::{precompile, PRECOMPILED_EXTENSION};

fn main() {
    let mut target = None;
    let mut modules: Vec<PathBuf> = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--target" => match arguments.next() {
                Some(triple) => target = Some(triple),
                None => usage(),
            },
            _ => modules.push(PathBuf::from(argument)),
        }
    }
    if modules.is_empty() {
        usage();
    }
    for module in &modules {
        if let Err(error) = compile(module, target.as_deref()) {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
    let described = target.as_deref().unwrap_or("this machine");
    println!("Compiled {} plugins for {described}", modules.len());
}

fn compile(module: &Path, target: Option<&str>) -> Result<(), String> {
    let bytes = std::fs::read(module)
        .map_err(|error| format!("{} could not be read: {error}", module.display()))?;
    let compiled = precompile(&bytes, target)
        .map_err(|error| format!("{} could not be compiled: {error}", module.display()))?;
    let artifact = module.with_extension(PRECOMPILED_EXTENSION);
    std::fs::write(&artifact, compiled)
        .map_err(|error| format!("{} could not be written: {error}", artifact.display()))
}

fn usage() -> ! {
    eprintln!("usage: precompile [--target TRIPLE] PLUGIN.wasm...");
    std::process::exit(2);
}
