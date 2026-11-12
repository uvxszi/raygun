//! Packs the parts of Python's standard library that Kodi add-ons use (CPython's `Lib`, as
//! RustPython ships it in `rustpython-pylib`) into `kodi-stdlib.zip`, compiled into the crate
//! (`kodi::python::files`). Packages and modules for windows, terminals, tests, processes and
//! the like are left out.

use std::io::Write;
use std::path::Path;

/// Packages kept whole.
const PACKAGES: &[&str] = &[
    "collections",
    "concurrent",
    "dbm",
    "email",
    "encodings",
    "html",
    "http",
    "importlib",
    "json",
    "logging",
    "pathlib",
    "re",
    "string",
    "sysconfig",
    "tomllib",
    "urllib",
    "xml",
    "xmlrpc",
    "zipfile",
    "wsgiref",
    "zoneinfo",
    "compression",
];

/// Single modules kept.
const MODULES: &[&str] = &[
    "__future__",
    "_collections_abc",
    "_colorize",
    "_compat_pickle",
    "_markupbase",
    "_opcode_metadata",
    "_py_abc",
    "_py_warnings",
    "_pydatetime",
    "_pydecimal",
    "_pyio",
    "_pylong",
    "_sitebuiltins",
    "_strptime",
    "_threading_local",
    "_weakrefset",
    "abc",
    "annotationlib",
    "argparse",
    "ast",
    "base64",
    "bisect",
    "bz2",
    "calendar",
    "cmd",
    "code",
    "codeop",
    "colorsys",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "csv",
    "dataclasses",
    "datetime",
    "decimal",
    "difflib",
    "dis",
    "enum",
    "filecmp",
    "fnmatch",
    "fractions",
    "functools",
    "genericpath",
    "getopt",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "inspect",
    "io",
    "ipaddress",
    "keyword",
    "linecache",
    "locale",
    "lzma",
    "mimetypes",
    "netrc",
    "ntpath",
    "nturl2path",
    "numbers",
    "opcode",
    "operator",
    "optparse",
    "os",
    "pickle",
    "pkgutil",
    "plistlib",
    "posixpath",
    "pprint",
    "profile",
    "pstats",
    "py_compile",
    "queue",
    "quopri",
    "random",
    "reprlib",
    "runpy",
    "sched",
    "secrets",
    "selectors",
    "shelve",
    "shlex",
    "socketserver",
    "shutil",
    "sre_compile",
    "sre_constants",
    "sre_parse",
    "stat",
    "statistics",
    "stringprep",
    "struct",
    "symtable",
    "tarfile",
    "tempfile",
    "textwrap",
    "this",
    "threading",
    "timeit",
    "token",
    "tokenize",
    "traceback",
    "types",
    "typing",
    "uuid",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "zipimport",
];

fn add_file(zip: &mut zip::ZipWriter<std::fs::File>, lib: &Path, relative: &str) {
    let path = lib.join(relative);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(relative, options).expect("zip entry");
    zip.write_all(&bytes).expect("zip write");
}

fn add_folder(zip: &mut zip::ZipWriter<std::fs::File>, lib: &Path, relative: &str) {
    let folder = lib.join(relative);
    let mut entries: Vec<_> = std::fs::read_dir(&folder)
        .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = format!("{relative}/{name}");
        let kind = entry.file_type().expect("file type");
        if kind.is_dir() {
            if name != "__pycache__" && name != "test" && name != "tests" {
                add_folder(zip, lib, &child);
            }
        } else if name.ends_with(".py") {
            add_file(zip, lib, &child);
        }
    }
}

fn main() {
    let lib = Path::new(rustpython_pylib::LIB_PATH);
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let target = Path::new(&out).join("kodi-stdlib.zip");
    let file = std::fs::File::create(&target).expect("create kodi-stdlib.zip");
    let mut zip = zip::ZipWriter::new(file);
    for package in PACKAGES {
        add_folder(&mut zip, lib, package);
    }
    for module in MODULES {
        add_file(&mut zip, lib, &format!("{module}.py"));
    }
    zip.finish().expect("finish kodi-stdlib.zip");
    println!("cargo:rerun-if-changed=build.rs");
}
