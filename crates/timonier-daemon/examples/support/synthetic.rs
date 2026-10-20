//! A synthetic Cargo workspace for the benches (PLAN.md 5.2, ADR 0024, 0025), written from its
//! size alone so that every figure can be made again from the repository.

use std::path::{Path, PathBuf};

/// A synthetic Cargo workspace, `size` = `CxDxF`: C crates `c<n>`, each a `src/lib.rs` and D
/// folders `d<n>` of F files `f<n>.rs` and a `mod.rs`. Each file calls the next one of its
/// folder, the first file of each folder a file of the next folder, and the first crate's
/// library is called by the next crate. Gives the number of files written.
pub fn write(size: &str, to: &Path) -> usize {
    let numbers: Vec<usize> = size
        .split('x')
        .map(|n| n.parse().expect("synthetic:CxDxF, three numbers"))
        .collect();
    let [crates, dirs, files] = numbers[..] else {
        panic!("synthetic:CxDxF, three numbers");
    };
    let mut written = 0;
    let mut write = |path: PathBuf, text: String| {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        std::fs::write(path, text).expect("write");
        written += 1;
    };
    let members: Vec<String> = (0..crates)
        .map(|c| format!("    \"crates/c{c}\""))
        .collect();
    write(
        to.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [\n{}\n]\n",
            members.join(",\n")
        ),
    );
    for c in 0..crates {
        let root = to.join(format!("crates/c{c}"));
        let dependency = if c == 0 {
            String::new()
        } else {
            format!(
                "\n[dependencies]\nc{} = {{ path = \"../c{}\" }}\n",
                c - 1,
                c - 1
            )
        };
        write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"c{c}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{dependency}"
            ),
        );
        let mut lib: String = (0..dirs).map(|d| format!("pub mod d{d};\n")).collect();
        lib.push_str("\npub fn entry() {\n");
        for d in 0..dirs {
            lib.push_str(&format!("    d{d}::f0::run();\n"));
        }
        lib.push_str("}\n");
        write(root.join("src/lib.rs"), lib);
        for d in 0..dirs {
            let folder = root.join(format!("src/d{d}"));
            write(
                folder.join("mod.rs"),
                (0..files).map(|f| format!("pub mod f{f};\n")).collect(),
            );
            for f in 0..files {
                let mut body = format!(
                    "// crate {c}, folder {d}, file {f}\npub fn run() {{\n    super::f{}::helper();\n",
                    (f + 1) % files
                );
                if f == 0 {
                    body.push_str(&format!("    crate::d{}::f1::helper();\n", (d + 1) % dirs));
                }
                if c > 0 && f == 0 && d == 0 {
                    body.push_str(&format!("    c{}::entry();\n", c - 1));
                }
                body.push_str("}\n\npub fn helper() {}\n");
                write(folder.join(format!("f{f}.rs")), body);
            }
        }
    }
    written
}
