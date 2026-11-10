use std::{path::PathBuf,process::Command};
fn main(){
    println!("cargo:rerun-if-changed=assets");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref()!=Ok("windows"){return;}
    let kits=PathBuf::from(std::env::var_os("ProgramFiles(x86)").expect("Windows SDK location unavailable")).join("Windows Kits/10/bin");
    let mut compilers=std::fs::read_dir(kits).expect("Windows SDK required on the cloud builder").filter_map(Result::ok).map(|entry|entry.path().join("x64/rc.exe")).filter(|path|path.exists()).collect::<Vec<_>>();compilers.sort();let compiler=compilers.last().expect("Windows SDK resource compiler unavailable");
    let output=PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("altrun.res");let assets=PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets");
    let status=Command::new(compiler).current_dir(assets).arg("/nologo").arg("/fo").arg(&output).arg("app.rc").status().expect("Could not run cloud resource compiler");assert!(status.success(),"Windows resources failed to compile");
    println!("cargo:rustc-link-arg-bin=altrun-probe={}",output.display());
}
