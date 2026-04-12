// Build script for koala-qt.
//
// Compiles the C++ widget layer (adapted from Ladybird) and links it against
// the Qt6 frameworks installed via Homebrew. The Rust side drives the build
// via a `cxx` bridge declared in `src/bridge.rs`.
//
// This file intentionally avoids CMake. Qt installation is discovered by
// shelling out to `qmake6`, and moc is run on any headers listed in
// MOC_HEADERS below. The compiled artefacts are linked into the Rust binary
// through `cxx-build`.

use std::path::PathBuf;
use std::process::Command;

// Headers containing Q_OBJECT classes. Each gets run through moc and the
// generated `.cpp` is added to the compile list.
const MOC_HEADERS: &[&str] = &[
    "cpp/BrowserWindow.h",
    "cpp/Tab.h",
    "cpp/LocationEdit.h",
    "cpp/TabBar.h",
    "cpp/BrowserView.h",
];

// Non-moc C++ source files compiled directly.
const CPP_SOURCES: &[&str] = &[
    "cpp/koala_window.cpp",
    "cpp/BrowserWindow.cpp",
    "cpp/Tab.cpp",
    "cpp/LocationEdit.cpp",
    "cpp/TabBar.cpp",
    "cpp/Icons.cpp",
    "cpp/BrowserView.cpp",
];

fn qmake_query(var: &str) -> String {
    let output = Command::new("qmake6")
        .args(["-query", var])
        .output()
        .unwrap_or_else(|e| panic!("failed to run qmake6 -query {var}: {e}"));
    assert!(
        output.status.success(),
        "qmake6 -query {var} exited with {:?}",
        output.status
    );
    String::from_utf8(output.stdout)
        .expect("qmake6 output was not utf-8")
        .trim()
        .to_owned()
}

fn main() {
    // On macOS, Homebrew installs Qt as frameworks under QT_INSTALL_LIBS
    // (e.g. `/opt/homebrew/lib/QtWidgets.framework`). The "headers" path
    // reported by qmake6 is unused in that layout — `#include <QApplication>`
    // resolves via `-F <libs>` because clang treats framework directories
    // as implicit header roots. On Linux we'd use `-I <headers>/QtWidgets`
    // instead; gate the two paths by target once we care about Linux.
    let qt_libs = PathBuf::from(qmake_query("QT_INSTALL_LIBS"));
    let qt_bins = PathBuf::from(qmake_query("QT_INSTALL_BINS"));
    let qt_libexecs = PathBuf::from(qmake_query("QT_INSTALL_LIBEXECS"));
    let qt_headers = PathBuf::from(qmake_query("QT_INSTALL_HEADERS"));

    // Qt 6 moved `moc`, `rcc`, `uic` out of QT_INSTALL_BINS and into
    // QT_INSTALL_LIBEXECS. Fall back to BINS for older layouts.
    let moc = {
        let libexec_moc = qt_libexecs.join("moc");
        if libexec_moc.exists() {
            libexec_moc
        } else {
            qt_bins.join("moc")
        }
    };
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Run moc on each header. The generated sources land in OUT_DIR.
    let mut moc_generated: Vec<PathBuf> = Vec::new();
    for header in MOC_HEADERS {
        let stem = PathBuf::from(header)
            .file_stem()
            .expect("header without file stem")
            .to_os_string();
        let mut out_name = stem;
        out_name.push(".moc.cpp");
        let out_path = out_dir.join(out_name);

        let status = Command::new(&moc)
            .arg(header)
            .arg("-o")
            .arg(&out_path)
            .arg("-F")
            .arg(&qt_libs)
            .status()
            .unwrap_or_else(|e| panic!("failed to run moc on {header}: {e}"));
        assert!(status.success(), "moc failed for {header}");

        moc_generated.push(out_path);
        println!("cargo:rerun-if-changed={header}");
    }

    let mut build = cxx_build::bridge("src/bridge.rs");
    build
        .std("c++20")
        .include("cpp")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-fPIC");

    // Ladybird's sources use unqualified Qt includes (`#include <QAction>`
    // rather than `<QtGui/QAction>`). For that to resolve we need each Qt
    // module's header directory on the include path explicitly. On macOS
    // with Homebrew Qt those live inside the framework bundles.
    let qt_frameworks = [
        "QtCore",
        "QtGui",
        "QtWidgets",
    ];
    for framework in qt_frameworks {
        let framework_headers = qt_libs
            .join(format!("{framework}.framework"))
            .join("Headers");
        if framework_headers.exists() {
            build.include(&framework_headers);
        } else {
            // Linux / non-framework layout: headers live under QT_INSTALL_HEADERS.
            let linux_headers = qt_headers.join(framework);
            if linux_headers.exists() {
                build.include(&linux_headers);
            }
        }
    }
    // Also add the umbrella headers dir as a fallback.
    if qt_headers.exists() {
        build.include(&qt_headers);
    }
    // Framework search path so `-framework QtWidgets` at link time resolves.
    build.flag("-F").flag(
        qt_libs
            .to_str()
            .expect("QT_INSTALL_LIBS is not valid utf-8"),
    );

    for src in CPP_SOURCES {
        build.file(src);
        println!("cargo:rerun-if-changed={src}");
    }
    for generated in &moc_generated {
        build.file(generated);
    }

    build.compile("koala_qt_cpp");

    // Qt frameworks live in QT_INSTALL_LIBS on macOS (Homebrew install).
    // We pass it as both a framework search path and a library search path.
    println!("cargo:rustc-link-search=framework={}", qt_libs.display());
    println!("cargo:rustc-link-search=native={}", qt_libs.display());
    println!("cargo:rustc-link-lib=framework=QtCore");
    println!("cargo:rustc-link-lib=framework=QtGui");
    println!("cargo:rustc-link-lib=framework=QtWidgets");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/bridge.rs");
    println!("cargo:rerun-if-changed=cpp/koala_window.h");
    // `include_str!` on the template files should cause rustc to
    // emit them as dependencies automatically, but we've seen
    // cargo's incremental detection miss the change in practice.
    // Declare them explicitly here so plain HTML edits always
    // trigger a rebuild.
    println!("cargo:rerun-if-changed=res/landing.html");
    println!("cargo:rerun-if-changed=res/error.html");
}
