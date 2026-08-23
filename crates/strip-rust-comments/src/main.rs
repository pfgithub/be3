use std::env;
use std::error::Error;
use std::path::Path;
use strip_rust_comments::strip_repository;

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
    strip_repository(Path::new("."), check)
}
