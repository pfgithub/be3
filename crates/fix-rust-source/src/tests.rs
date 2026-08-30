use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("fix-rust-source-{}-{sequence}", std::process::id()))
}

mod fix_repository_check_reports_without_changes;
mod fix_repository_enforces_rust_layout;
mod fix_repository_removes_test_path_attributes;
mod fix_repository_skips_hidden_directories;
mod strip_comments;
