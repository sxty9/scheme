//! Build-Skript: kompiliert die gRPC-Service-Definition (`proto/scheme.proto`) mit
//! `tonic-prost-build` zu Rust-Code.
//!
//! In dieser Umgebung ist **kein** System-`protoc` installiert. Wir setzen daher
//! `PROTOC` auf das von `protoc-bin-vendored` mitgelieferte, vorgebaute Binär — so
//! braucht weder `tonic-build`/`prost-build` noch der CI ein System-`protoc`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // SAFETY: build.rs läuft single-threaded vor jeder Nebenläufigkeit; das Setzen
    // von PROTOC ist der dokumentierte Weg, prost/tonic ein protoc vorzugeben.
    unsafe {
        std::env::set_var("PROTOC", &protoc);
    }

    println!("cargo:rerun-if-changed=proto/scheme.proto");
    println!("cargo:rerun-if-changed=build.rs");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/scheme.proto"], &["proto"])?;
    Ok(())
}
