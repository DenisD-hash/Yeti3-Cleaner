mod config;
mod history;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use humansize::{BINARY, format_size};
use std::{
    ffi::CString,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime},
};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(
    name = "yeti3-cleaner",
    version,
    about = "Yeti3-Cleaner — aggressive but safe macOS cleaner"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Scan(Opts),
    Clean(CleanOpts),

    /// Latest cleanup result for the macOS UI
    Result,
}

#[derive(Args, Clone)]
struct Opts {
    #[arg(long)]
    deep: bool,

    #[arg(long)]
    dev: bool,

    /// Full safe cleanup analysis
    #[arg(long)]
    max: bool,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Args)]
struct CleanOpts {
    #[command(flatten)]
    opts: Opts,

    /// Show actions without deleting
    #[arg(long)]
    dry_run: bool,

    /// Skip confirmation
    #[arg(long)]
    yes: bool,
}

#[derive(Clone)]
struct Candidate {
    label: &'static str,
    path: PathBuf,
    root: PathBuf,
    bytes: u64,
    min_age: u64,
}

struct Disk {
    total: u64,
    free: u64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Cmd::Scan(o) => scan_command(&o),
        Cmd::Clean(o) => clean_command(&o),
        Cmd::Result => {
            println!("{}", history::latest_result_json()?);
            Ok(())
        }
    }
}

fn home() -> Result<PathBuf> {
    dirs::home_dir().context("Cannot determine HOME")
}

fn roots(o: &Opts) -> Result<Vec<Candidate>> {
    let h = home()?;
    let mut v = Vec::new();

    let mut add = |label, rel: &str, age| {
        let p = h.join(rel);
        v.push(Candidate {
            label,
            path: p.clone(),
            root: p,
            bytes: 0,
            min_age: age,
        });
    };

    add("Trash", ".Trash", 0);
    add("Application caches", "Library/Caches", 1);
    add("Logs", "Library/Logs", 2);

    if o.deep || o.max {
        add("Crash reports", "Library/Logs/DiagnosticReports", 1);
        add("AppSupport caches", "Library/Application Support/Caches", 1);
        add("CoreML cache", "Library/Application Support/coreMLCache", 1);
    }

    if o.dev || o.max {
        add(
            "Xcode DerivedData",
            "Library/Developer/Xcode/DerivedData",
            0,
        );
        add(
            "Xcode SourcePackages",
            "Library/Developer/Xcode/SourcePackages",
            7,
        );
        add("Cargo cache", ".cargo/registry/cache", 0);
        add("Cargo sources", ".cargo/registry/src", 7);
        add("Cargo git cache", ".cargo/git/checkouts", 7);
        add("npm cache", ".npm/_cacache", 0);
        add("npx cache", ".npm/_npx", 7);
        add("pip cache", "Library/Caches/pip", 0);
        add("uv cache", "Library/Caches/uv", 0);
        add("uv cache", ".cache/uv", 0);
        add("Yarn cache", "Library/Caches/Yarn", 0);
        add("pnpm cache", "Library/Caches/pnpm", 0);
        add("CocoaPods cache", "Library/Caches/CocoaPods", 0);
        add("Gradle cache", ".gradle/caches", 14);
    }

    Ok(v)
}

fn collect(o: &Opts) -> Result<Vec<Candidate>> {
    let mut result = Vec::new();

    for root in roots(o)? {
        if !root.path.is_dir() {
            continue;
        }

        let rd = match fs::read_dir(&root.path) {
            Ok(x) => x,
            Err(_) => continue,
        };

        for e in rd.flatten() {
            let p = e.path();

            let m = match fs::symlink_metadata(&p) {
                Ok(x) => x,
                Err(_) => continue,
            };

            if m.file_type().is_symlink() {
                continue;
            }

            let age = age_days(&p).unwrap_or(0);
            if age < root.min_age {
                continue;
            }

            let bytes = tree_size(&p);
            if bytes == 0 {
                continue;
            }

            result.push(Candidate {
                label: root.label,
                path: p,
                root: root.path.clone(),
                bytes,
                min_age: root.min_age,
            });
        }
    }

    result.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    Ok(result)
}

fn scan_command(o: &Opts) -> Result<()> {
    let disk = disk("/")?;
    let files = collect(o)?;
    let regular = total(&files);

    let mobile = if o.max {
        tree_size(&home()?.join("Library/Application Support/MobileSync/Backup"))
    } else {
        0
    };

    println!("YETI³ CLEANER");
    println!("────────────────────────────────────────────");
    println!("Total disk               {:>16}", human(disk.total));
    println!(
        "Used                     {:>16}",
        human(disk.total.saturating_sub(disk.free))
    );
    println!("Free                     {:>16}", human(disk.free));
    println!();

    println!("CLEANABLE");
    println!("Regular                   {:>16}", human(regular));

    if o.max {
        println!("MobileSync backups        {:>16}", human(mobile));

        println!();
        println!("SPECIAL CLEANERS");
        print_special_status();
    }

    println!("────────────────────────────────────────────");
    println!(
        "Known direct total        {:>16}",
        human(regular.saturating_add(mobile))
    );

    if o.verbose {
        println!();
        for c in &files {
            println!(
                "{:>12}  {:<22} {}",
                human(c.bytes),
                c.label,
                c.path.display()
            );
        }
    }

    println!();
    println!("READ-ONLY MODE — nothing was deleted.");
    Ok(())
}

fn clean_command(c: &CleanOpts) -> Result<()> {
    let started = Instant::now();
    let before = disk("/")?;
    let files = collect(&c.opts)?;

    let mobile = home()?.join("Library/Application Support/MobileSync/Backup");

    let mobile_bytes = if c.opts.max { tree_size(&mobile) } else { 0 };

    let regular = total(&files);
    let direct = regular.saturating_add(mobile_bytes);

    let history = history::HistoryDb::open()?;
    let run_id = history.begin_cleanup(
        before.total,
        before.free,
        direct,
        if c.opts.max {
            "max"
        } else if c.opts.deep {
            "deep"
        } else {
            "standard"
        },
    )?;

    println!("YETI³ CLEANER — CLEAN PLAN");
    println!("────────────────────────────────────────────");
    println!("Regular                   {:>16}", human(regular));

    if c.opts.max {
        println!("ALL MobileSync backups    {:>16}", human(mobile_bytes));
        println!("Homebrew                  managed cleanup");
        println!("Docker                    cache/images/containers/networks");
        println!("Docker volumes            PROTECTED");
        println!("Xcode unavailable sims    cleanup");
    }

    println!("────────────────────────────────────────────");
    println!("Direct known              {:>16}", human(direct));

    if c.dry_run {
        println!();
        println!("DRY RUN");

        for x in &files {
            println!("DELETE  {:>12}  {}", human(x.bytes), x.path.display());
        }

        if c.opts.max && mobile_bytes > 0 {
            println!("DELETE  {:>12}  {}", human(mobile_bytes), mobile.display());
        }

        println!();
        println!("External cleaners would also run in preview/managed mode.");
        return Ok(());
    }

    if !c.yes {
        print!("\nType YES to continue: ");
        io::stdout().flush()?;

        let mut s = String::new();
        io::stdin().read_line(&mut s)?;

        if s.trim() != "YES" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let mut removed = 0u64;
    let mut failures = 0usize;
    let mut files_deleted = 0u64;
    let mut dirs_deleted = 0u64;
    let mut skipped = 0u64;

    println!("\n===== FILE CLEANERS =====");

    for x in &files {
        let was_dir = x.path.is_dir();

        match validate(&x.path, &x.root).and_then(|_| remove(&x.path)) {
            Ok(_) => {
                removed = removed.saturating_add(x.bytes);

                if was_dir {
                    dirs_deleted = dirs_deleted.saturating_add(1);
                } else {
                    files_deleted = files_deleted.saturating_add(1);
                }

                history.add_cleanup_entry(&history::CleanupEntry {
                    run_id,
                    category: x.label.to_string(),
                    path: x.path.clone(),
                    parent_path: history::parent_path(&x.path),
                    kind: if was_dir { "directory" } else { "file" }.to_string(),
                    bytes_before: x.bytes,
                    bytes_reclaimed: x.bytes,
                    action: "delete".to_string(),
                    rule: format!("age >= {} days", x.min_age),
                    result: "deleted".to_string(),
                    error: None,
                })?;

                if c.opts.verbose {
                    println!("REMOVED {}", x.path.display());
                }
            }

            Err(e) => {
                failures += 1;
                skipped = skipped.saturating_add(1);

                history.add_cleanup_entry(&history::CleanupEntry {
                    run_id,
                    category: x.label.to_string(),
                    path: x.path.clone(),
                    parent_path: history::parent_path(&x.path),
                    kind: if was_dir { "directory" } else { "file" }.to_string(),
                    bytes_before: x.bytes,
                    bytes_reclaimed: 0,
                    action: "delete".to_string(),
                    rule: format!("age >= {} days", x.min_age),
                    result: "error".to_string(),
                    error: Some(e.to_string()),
                })?;

                eprintln!("SKIP {}: {}", x.path.display(), e);
            }
        }
    }

    if c.opts.max {
        println!("\n===== MOBILESYNC =====");

        if mobile.is_dir() {
            // User policy: remove ALL local iPhone/iPad backups.
            let backup_root = home()?.join("Library/Application Support/MobileSync");

            match validate(&mobile, &backup_root).and_then(|_| empty_directory(&mobile)) {
                Ok(_) => {
                    removed = removed.saturating_add(mobile_bytes);
                    dirs_deleted = dirs_deleted.saturating_add(1);

                    history.add_cleanup_entry(&history::CleanupEntry {
                        run_id,
                        category: "iPhone / iPad".to_string(),
                        path: mobile.clone(),
                        parent_path: history::parent_path(&mobile),
                        kind: "directory".to_string(),
                        bytes_before: mobile_bytes,
                        bytes_reclaimed: mobile_bytes,
                        action: "empty".to_string(),
                        rule: "remove all local device backups".to_string(),
                        result: "deleted".to_string(),
                        error: None,
                    })?;

                    println!("Local iPhone/iPad backups removed.");
                }

                Err(e) => {
                    failures += 1;
                    skipped = skipped.saturating_add(1);

                    history.add_cleanup_entry(&history::CleanupEntry {
                        run_id,
                        category: "iPhone / iPad".to_string(),
                        path: mobile.clone(),
                        parent_path: history::parent_path(&mobile),
                        kind: "directory".to_string(),
                        bytes_before: mobile_bytes,
                        bytes_reclaimed: 0,
                        action: "empty".to_string(),
                        rule: "remove all local device backups".to_string(),
                        result: "error".to_string(),
                        error: Some(e.to_string()),
                    })?;

                    eprintln!("MobileSync skipped: {e}");
                }
            }
        }

        println!("\n===== HOMEBREW =====");
        run_managed("brew", &["autoremove"]);
        run_managed("brew", &["cleanup", "--prune=all"]);

        println!("\n===== DOCKER =====");

        if command_ok("docker", &["info"]) {
            // Intentionally no `docker volume prune`.
            run_managed("docker", &["builder", "prune", "-af"]);
            run_managed("docker", &["image", "prune", "-f"]);
            run_managed("docker", &["container", "prune", "-f"]);
            run_managed("docker", &["network", "prune", "-f"]);

            println!("Docker volumes preserved.");
        } else {
            println!("Docker daemon unavailable — skipped.");
        }

        println!("\n===== XCODE =====");

        if command_exists("xcrun") {
            run_managed("xcrun", &["simctl", "shutdown", "all"]);
            run_managed("xcrun", &["simctl", "delete", "unavailable"]);
        }
    }

    let after = disk("/")?;
    let actual = after.free.saturating_sub(before.free);
    let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;

    history.finish_cleanup(
        run_id,
        after.free,
        actual,
        files_deleted,
        dirs_deleted,
        skipped,
        failures as u64,
        elapsed_ms,
        if failures == 0 {
            "completed"
        } else {
            "completed_with_errors"
        },
    )?;

    history::write_latest_result()?;

    println!();
    println!("YETI³ CLEANER — COMPLETE");
    println!("────────────────────────────────────────────");
    println!("Free before              {:>16}", human(before.free));
    println!("Free after               {:>16}", human(after.free));
    println!("Direct deleted estimate  {:>16}", human(removed));
    println!("Actually reclaimed       {:>16}", human(actual));
    println!("Failed/skipped            {:>16}", failures);

    Ok(())
}

fn print_special_status() {
    if command_exists("brew") {
        println!("Homebrew                  available");
    } else {
        println!("Homebrew                  not installed");
    }

    if command_ok("docker", &["info"]) {
        println!("Docker                    available");
    } else {
        println!("Docker                    daemon unavailable");
    }

    if command_exists("xcrun") {
        println!("Xcode/simctl              available");
    } else {
        println!("Xcode/simctl              unavailable");
    }
}

fn run_managed(program: &str, args: &[&str]) {
    println!("$ {} {}", program, args.join(" "));

    match Command::new(program).args(args).status() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("{program} exited with {status}"),
        Err(e) => eprintln!("{program}: {e}"),
    }
}

fn command_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn command_exists(program: &str) -> bool {
    Command::new("which")
        .arg(program)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn validate(path: &Path, root: &Path) -> Result<()> {
    let h = home()?;

    if path == Path::new("/") || path == h || path == root {
        anyhow::bail!("protected root");
    }

    let m = fs::symlink_metadata(path)?;

    if m.file_type().is_symlink() {
        anyhow::bail!("symlink refused");
    }

    let cp = fs::canonicalize(path)?;
    let cr = fs::canonicalize(root)?;

    if !cp.starts_with(&cr) || cp == cr {
        anyhow::bail!("outside allowlisted root");
    }

    let protected = [
        h.join("Desktop"),
        h.join("Documents"),
        h.join("Downloads"),
        h.join("Movies"),
        h.join("Music"),
        h.join("Pictures"),
        h.join("Library/Mobile Documents"),
        h.join("Library/CloudStorage"),
        h.join("Library/Keychains"),
        h.join("Library/Mail"),
        h.join("Library/Messages"),
    ];

    if protected.iter().any(|p| cp == *p || cp.starts_with(p)) {
        anyhow::bail!("protected user data");
    }

    Ok(())
}

fn empty_directory(dir: &Path) -> Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        remove(&p)?;
    }
    Ok(())
}

fn remove(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;

    if m.file_type().is_symlink() {
        anyhow::bail!("symlink refused");
    }

    if m.is_dir() {
        fs::remove_dir_all(path)?;
    } else if m.is_file() {
        fs::remove_file(path)?;
    }

    Ok(())
}

fn age_days(path: &Path) -> Option<u64> {
    let t = fs::symlink_metadata(path).ok()?.modified().ok()?;
    let d = SystemTime::now().duration_since(t).ok()?;
    Some(d.as_secs() / 86400)
}

fn tree_size(path: &Path) -> u64 {
    let m = match fs::symlink_metadata(path) {
        Ok(x) => x,
        Err(_) => return 0,
    };

    if m.file_type().is_symlink() {
        return 0;
    }

    if m.is_file() {
        return m.len();
    }

    WalkDir::new(path)
        .follow_links(false)
        .same_file_system(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let m = e.metadata().ok()?;

            if m.file_type().is_symlink() {
                None
            } else if m.is_file() {
                Some(m.len())
            } else {
                None
            }
        })
        .fold(0u64, |a, b| a.saturating_add(b))
}

fn total(v: &[Candidate]) -> u64 {
    v.iter().fold(0u64, |a, x| a.saturating_add(x.bytes))
}

fn disk(path: &str) -> Result<Disk> {
    let c = CString::new(path)?;
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };

    if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let bs = s.f_bsize as u64;

    Ok(Disk {
        total: s.f_blocks.saturating_mul(bs),
        free: s.f_bavail.saturating_mul(bs),
    })
}

fn human(n: u64) -> String {
    format_size(n, BINARY)
}
