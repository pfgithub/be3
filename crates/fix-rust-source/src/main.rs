use fix_rust_source::fix_repository;
use std::env;
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let check = match arguments.next().as_deref() {
        None => false,
        Some("--check") => true,
        Some(argument) => return Err(format!("unknown argument: {argument}").into()),
    };
    if let Some(argument) = arguments.next() {
        return Err(format!("unknown argument: {argument}").into());
    }
    fix_repository(Path::new("."), check)
}
