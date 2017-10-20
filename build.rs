use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target = env::var("TARGET").unwrap();

    if target.contains("x86_64") {
    } else {
        for file in &["asm/thunk_pre_x86.S"] {
            let (prefix, flags) = if target.contains("windows") {
                ("i686-w64-mingw32-", &[][..])
            } else {
                ("", &["-m32"][..])
            };

            println!("cargo:rerun-if-changed={}", file);

            let out = Path::new(&out_dir).join(format!("{}.bin", Path::new(Path::new(file).file_stem().unwrap()).display()));
            command(Command::new(format!("{}as", prefix))
                .arg(file)
                .args(flags)
                .arg("-o").arg(&out)
            );
            command(Command::new(format!("{}objcopy", prefix))
                .arg("-O").arg("binary")
                .arg(&out)
            );
        }
    }
}

fn command(cmd: &mut Command) {
    assert!(cmd.status().unwrap().success());
}
