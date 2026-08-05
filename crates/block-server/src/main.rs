use std::{env, path::PathBuf, process::ExitCode};

use block_server::ServerConfig;
use tokio::net::TcpListener;

const DEFAULT_ADDR: &str = "127.0.0.1:9090";
const DEFAULT_DATA_DIR: &str = "block-data";

const USAGE: &str = "\
Usage:
  block-server [--addr ADDR] [--data-dir DIR] [--disable-registration]
  block-server add-account --data-dir DIR --email EMAIL --display-name NAME

  --addr ADDR                 Address to listen on (default: 127.0.0.1:9090)
  --data-dir DIR               Where block and account data is stored (default: block-data)
  --disable-registration       Refuse ManagementClientMessage::Register; use
                                `add-account` to provision accounts by hand
                                instead. Meant for a publicly hosted server.

  add-account prompts for the new account's password on the terminal and
  creates it directly in the database at DIR, bypassing --disable-registration.";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = if args.first().map(String::as_str) == Some("add-account") {
        run_add_account(&args[1..]).await
    } else {
        run_serve(&args).await
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run_serve(args: &[String]) -> Result<(), String> {
    let mut addr = DEFAULT_ADDR.to_string();
    let mut data_dir = PathBuf::from(DEFAULT_DATA_DIR);
    let mut disable_registration = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => addr = next_value(&mut args, "--addr")?,
            "--data-dir" => data_dir = PathBuf::from(next_value(&mut args, "--data-dir")?),
            "--disable-registration" => disable_registration = true,
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unrecognized argument {other}\n\n{USAGE}")),
        }
    }

    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|error| format!("failed to bind {addr}: {error}"))?;
    let config = ServerConfig {
        allow_registration: !disable_registration,
    };
    println!(
        "block server listening on http://{addr} storing data in {}{}",
        data_dir.display(),
        if disable_registration {
            " (registration disabled)"
        } else {
            ""
        }
    );
    block_server::serve_with_config(listener, data_dir, config)
        .await
        .map_err(|error| error.to_string())
}

async fn run_add_account(args: &[String]) -> Result<(), String> {
    let mut data_dir = None;
    let mut email = None;
    let mut display_name = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => data_dir = Some(PathBuf::from(next_value(&mut args, "--data-dir")?)),
            "--email" => email = Some(next_value(&mut args, "--email")?),
            "--display-name" => display_name = Some(next_value(&mut args, "--display-name")?),
            "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unrecognized argument {other}\n\n{USAGE}")),
        }
    }
    let data_dir = data_dir.ok_or_else(|| format!("--data-dir is required\n\n{USAGE}"))?;
    let email = email.ok_or_else(|| format!("--email is required\n\n{USAGE}"))?;
    let display_name =
        display_name.ok_or_else(|| format!("--display-name is required\n\n{USAGE}"))?;

    let password = rpassword::prompt_password("Password: ").map_err(|error| error.to_string())?;
    let confirmation =
        rpassword::prompt_password("Confirm password: ").map_err(|error| error.to_string())?;
    if password != confirmation {
        return Err("passwords did not match".into());
    }

    let account = block_server::add_account(data_dir, email, display_name, password)
        .await
        .map_err(|error| error.to_string())?;
    println!("created account {} <{}>", account.id, account.email);
    Ok(())
}

fn next_value(args: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<String, String> {
    args.next()
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value\n\n{USAGE}"))
}
