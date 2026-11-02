fn main() {
    for name in ["TURBOJPEG_DYNAMIC", "TURBOJPEG_STATIC", "TURBOJPEG_SOURCE"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if std::env::var_os("CARGO_FEATURE_MOZJPEG").is_some() {
        assert_eq!(
            std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
            Ok("linux"),
            "experimental mozjpeg coexistence is currently verified only on Linux"
        );
        assert!(
            std::env::var("TURBOJPEG_DYNAMIC").as_deref() == Ok("1")
                && std::env::var("TURBOJPEG_STATIC").as_deref() != Ok("1")
                && std::env::var("TURBOJPEG_SOURCE")
                    .map_or(true, |source| source.eq_ignore_ascii_case("vendor")),
            "mozjpeg requires TURBOJPEG_DYNAMIC=1 and bundled TurboJPEG; static jpeg_* symbols collide. Bundle the resulting libturbojpeg.so.0 beside the executable and set its runtime search path; see README.md"
        );
    }
}
