extern crate json;


use std::process::{Command, Stdio};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::from_utf8;

static TARGET_MUTAGEN: &'static str = "target/mutagen";
static MUTATIONS_LIST: &'static str = "mutations.txt";

fn run_mutation(test_executable: &Path, mutation_count: usize) -> Result<String, String> {
    let output = Command::new(test_executable)
        // 0 is actually no mutations so we need i + 1 here
        .env("MUTATION_COUNT", mutation_count.to_string())
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(stdout)
    }
}

fn run_mutations(test_executable: &Path, list: Vec<String>) {
    let max_mutation = list.len();

    let mut failures = Vec::new();

    println!("Running {} mutations", max_mutation);
    for i in 0..max_mutation {
        // Mutation count starts from 1 (0 is not mutations)
        let mutation_count = i + 1;

        print!("{} ({})", list[i], mutation_count);

        let result = run_mutation(test_executable, mutation_count);

        if let Ok(stdout) = result {
            // A succeeding test suite is actually a failure for us.
            // At least on test should have failed
            failures.push((&list[i], mutation_count, stdout));
            println!(" ... FAILED");
        } else {
            println!(" ... ok");
        }
    }

    if !failures.is_empty() {
        println!("\nFailures:\n");

        for &(ref mutation, ref m_count, ref failure) in &failures {
            println!("---- {} ({}) stdout ----", mutation, m_count);
            for line in failure.split("\n") {
                println!("  {}", line);
            }
            println!("");
        }

        println!("\nFailures:\n");

        for &(mutation, m_count, _) in &failures {
            println!("\t{} ({})", mutation, m_count);
        }
    }

    println!(
        "\nMutation results: {}. {} passed; {} failed",
        if failures.is_empty() { "ok" } else { "FAILED" },
        list.len() - failures.len(),
        failures.len()
    );
}

fn get_mutations_filename() -> PathBuf {
    let metadata = Command::new("cargo")
        .arg("metadata")
        .output()
        .expect("failed to fetch metadata. Is this a Rust project?");
    if !metadata.status.success() {
        println!("failed to fetch metadata, cargo returned non-zero status.");
        panic!("{}", from_utf8(&metadata.stderr).unwrap());
    }
    let meta_json = json::parse(from_utf8(&metadata.stdout)
                                                   .expect("non-UTF8 cargo output"))
                                  .unwrap();
    let root_dir = Path::new(meta_json["workspace_root"]
                                  .as_str()
                                  .expect("cargo metadata misses workspace_root"));
    let mutagen_dir = root_dir.join(TARGET_MUTAGEN);
    if !mutagen_dir.exists() {
        panic!("Mutations are missing");
    }
    mutagen_dir.join(MUTATIONS_LIST)
}

fn compile_tests() -> PathBuf {
    let compile_out = Command::new("cargo")
        .args(&["test", "--no-run", "--message-format=json"])
        .stderr(Stdio::inherit())
        .output()
        .expect("Could not compile test");

    if !compile_out.status.success() {
        panic!("cargo test returned non-zero status");
    }
    let compile_stdout = from_utf8(&compile_out.stdout).expect("non-utf8 in cargo test messages");
    for line in compile_stdout.lines() {
        let msg_json = json::parse(line).expect(compile_stdout);
        if msg_json["reason"].as_str().unwrap() == "compiler-artifact"
            && msg_json["profile"]["test"].as_bool().unwrap_or(false) {
            for filename in msg_json["filenames"].members() {
                let f = filename.as_str().unwrap();
                if !f.ends_with(".rlib") {
                    return f.to_string().into()
                }
            }
        }
    }
    panic!("executable path not found");
}

fn read_mutations(filename: PathBuf) -> Vec<String> {
    let mut file = File::open(filename).expect("Mutations are missing");
    let mut s = String::new();
    file.read_to_string(&mut s)
        .expect("Failed reading mutations file");
    s.split("\n")
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

fn main() {
    let test_executable = compile_tests();
    println!("test executable at {:?}", test_executable);
    let filename = get_mutations_filename();
    let list = read_mutations(filename);
    if let Err(_) = run_mutation(&test_executable, 0){
        println!("You need to make sure you don't have failing tests before running 'cargo mutagen'");
        return;
    }
    run_mutations(&test_executable, list)
}
