// tests/cli_tests.rs
use assert_cmd::Command;
use predicates::{prelude::PredicateBooleanExt, str::contains};
use std::fs;
use tempfile::tempdir;

fn bin() -> Command {
    Command::cargo_bin("fpr").expect("binary built")
}

#[test]
fn prints_two_files_with_separator() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file1 = dir.path().join("foo.txt");
    let file2 = dir.path().join("bar.txt");
    fs::write(&file1, "hello")?;
    fs::write(&file2, "world")?;

    bin()
        .current_dir(dir.path())
        .args(["foo.txt", "bar.txt"])
        .assert()
        .success()
        .stdout(contains("=== foo.txt ==="))
        .stdout(contains("hello"))
        .stdout(contains("---"))
        .stdout(contains("=== bar.txt ==="))
        .stdout(contains("world"));
    Ok(())
}

#[test]
fn prints_with_glob_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let src = dir.path().join("src");
    fs::create_dir_all(&src)?;
    let file = src.join("main.rs");
    fs::write(&file, "fn main() {}")?;

    bin()
        .current_dir(dir.path())
        .arg("**/*.rs")
        .assert()
        .success()
        .stdout(contains("main.rs"))
        .stdout(contains("fn main() {}"));
    Ok(())
}

#[test]
fn prints_grouped_paths() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let src = dir.path().join("src");
    fs::create_dir_all(&src)?;
    let foo = src.join("foo.txt");
    let bar = src.join("bar.txt");
    fs::write(&foo, "foo")?;
    fs::write(&bar, "bar")?;

    bin()
        .current_dir(dir.path())
        .arg("src/(foo.txt, bar.txt)")
        .assert()
        .success()
        .stdout(contains("foo"))
        .stdout(contains("bar"));
    Ok(())
}

#[test]
fn exclusion_in_group() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let src = dir.path().join("src");
    fs::create_dir_all(&src)?;
    let keep = src.join("keep.txt");
    let drop = src.join("drop.txt");
    fs::write(&keep, "keep")?;
    fs::write(&drop, "drop")?;

    bin()
        .current_dir(dir.path())
        .arg("src/(keep.txt, -drop.txt)")
        .assert()
        .success()
        .stdout(contains("keep"))
        .stdout(contains("drop").not());
    Ok(())
}