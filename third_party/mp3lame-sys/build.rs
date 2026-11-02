//! Builds LAME's encoder with `cc` for every target. Raygun's change to upstream (which ran
//! autotools on Unix hosts and copied `configMS.h` on Windows hosts): the configuration is one
//! header, `lame-3.100/raygun_config.h`, so cross-compiling to Android from Windows works.

const LAME_DIR: &str = "lame-3.100";

fn main() {
    let lame = std::path::Path::new(LAME_DIR);
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set"));
    let config_dir = out_dir.join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    std::fs::copy(lame.join("raygun_config.h"), config_dir.join("config.h")).expect("config.h");
    println!("cargo:rerun-if-changed={LAME_DIR}/raygun_config.h");
    println!("cargo:rerun-if-changed=build.rs");

    let sources = [
        "bitstream.c",
        "encoder.c",
        "fft.c",
        "gain_analysis.c",
        "id3tag.c",
        "lame.c",
        "newmdct.c",
        "presets.c",
        "psymodel.c",
        "quantize_pvt.c",
        "vector/xmm_quantize_sub.c",
        "quantize.c",
        "reservoir.c",
        "set_get.c",
        "tables.c",
        "takehiro.c",
        "util.c",
        "vbrquantize.c",
        "VbrTag.c",
        "version.c",
    ];
    let mut build = cc::Build::new();
    for source in sources {
        build.file(lame.join("libmp3lame").join(source));
    }
    build
        .include(lame.join("include"))
        .include(&config_dir)
        .include(lame.join("libmp3lame"))
        .define("TAKEHIRO_IEEE754_HACK", None)
        .define("FLOAT8", Some("float"))
        .define("REAL_IS_FLOAT", Some("1"))
        .define("BS_FORMAT", Some("BINARY"))
        .define("HAVE_CONFIG_H", None)
        .warnings(false)
        .extra_warnings(false);
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("msvc") {
        build.flag_if_supported("/W0");
    } else {
        build.flag_if_supported("-w");
    }
    build.compile("mp3lame");
}
