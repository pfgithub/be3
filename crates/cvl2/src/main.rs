use std::{env, fs, process::ExitCode};

use cvl2::{import_file, pretty_print_errors, render_tokenized_output, tokenize, Source};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let [filename] = args.as_slice() else {
        eprintln!("Usage: cvl2 <file.qxc>");
        return ExitCode::FAILURE;
    };

    let contents = match fs::read_to_string(filename) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("failed to read {filename}: {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut source = Source::new(filename.as_str(), contents.as_str());
    let tokenized = tokenize(&mut source);
    println!("{}", render_tokenized_output(&tokenized, &source));

    match import_file(filename, &contents) {
        Ok(()) => ExitCode::SUCCESS,
        Err(errors) => {
            let source = Source::new(filename.as_str(), contents.as_str());
            println!("{}", pretty_print_errors(&source, &errors));
            ExitCode::FAILURE
        }
    }
}
