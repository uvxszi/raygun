//! The interpreter's ways out, written in Rust: `_raygun` (files, the network, the log, time),
//! `_raygun_sqlite` (SQLite), and `posix` (the `os` module's base, filled from
//! `lib/_raygun_posix.py`). Each finds its add-on's [`Sandbox`] on the `_raygun` module.

use std::sync::Arc;

use rustpython_vm::builtins::PyBaseExceptionRef;
use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine};

use super::Sandbox;

/// The errno Python raises for a failed file call (`FileNotFoundError` for 2...).
fn errno_of(error: &std::io::Error) -> i32 {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => 2,
        ErrorKind::PermissionDenied => 13,
        ErrorKind::AlreadyExists => 17,
        ErrorKind::CrossesDevices => 18,
        ErrorKind::NotADirectory => 20,
        ErrorKind::IsADirectory => 21,
        ErrorKind::InvalidInput => 22,
        ErrorKind::StorageFull => 28,
        ErrorKind::ReadOnlyFilesystem => 30,
        ErrorKind::DirectoryNotEmpty => 39,
        _ => 5,
    }
}

pub(crate) fn os_error(vm: &VirtualMachine, error: &std::io::Error) -> PyBaseExceptionRef {
    vm.new_errno_error(errno_of(error), error.to_string())
        .upcast()
}

/// The add-on's sandbox, kept on `_raygun._sandbox`.
pub(crate) fn sandbox(vm: &VirtualMachine) -> PyResult<Arc<Sandbox>> {
    let module = vm.import("_raygun", 0)?;
    let object = module.get_attr("_sandbox", vm)?;
    object
        .downcast_ref::<raygun::SandboxRef>()
        .map(|found| found.inner.clone())
        .ok_or_else(|| vm.new_runtime_error("Raygun's sandbox is missing"))
}

#[rustpython_vm::pymodule(name = "_raygun")]
pub(crate) mod raygun {
    use std::sync::Arc;
    use std::time::Duration;

    use rustpython_vm::builtins::{PyTupleRef, PyUtf8StrRef};
    use rustpython_vm::function::ArgBytesLike;
    use rustpython_vm::{PyObjectRef, PyPayload, PyResult, TryFromObject, VirtualMachine, pyclass};

    use super::super::files::Found;
    use super::super::net::FetchError;
    use super::super::{Sandbox, TIMED_OUT};
    use super::{os_error, sandbox};

    #[pyattr]
    #[pyclass(module = "_raygun", name = "Sandbox")]
    #[derive(PyPayload)]
    pub(crate) struct SandboxRef {
        pub(crate) inner: Arc<Sandbox>,
    }

    impl std::fmt::Debug for SandboxRef {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Sandbox")
        }
    }

    #[pyclass]
    impl SandboxRef {}

    impl SandboxRef {
        pub(crate) fn new(inner: Arc<Sandbox>) -> Self {
            Self { inner }
        }
    }

    fn strings(vm: &VirtualMachine, items: Vec<String>) -> PyObjectRef {
        vm.ctx
            .new_list(
                items
                    .into_iter()
                    .map(|item| vm.ctx.new_str(item).into())
                    .collect(),
            )
            .into()
    }

    #[pyfunction]
    fn find_module(
        fullname: PyUtf8StrRef,
        path: PyObjectRef,
        extra: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyObjectRef> {
        let sandbox = sandbox(vm)?;
        let fullname = fullname.as_str();
        let last = fullname.rsplit('.').next().unwrap_or(fullname);
        let folders = |list: &PyObjectRef| -> PyResult<Vec<String>> {
            if vm.is_none(list) {
                return Ok(Vec::new());
            }
            vm.extract_elements_with(list, |item| String::try_from_object(vm, item))
        };
        let found = if !vm.is_none(&path) {
            sandbox.files.find_in(&folders(&path)?, last)
        } else if fullname.contains('.') {
            None
        } else {
            // Folders the add-on put on `sys.path`, then the usual places.
            let added = folders(&extra)?;
            match sandbox.files.find_in(&added, last) {
                Some(Found::Namespace(mut portions)) => match sandbox.find_top(last) {
                    Some(Found::Namespace(more)) => {
                        portions.extend(more);
                        Some(Found::Namespace(portions))
                    }
                    Some(regular) => Some(regular),
                    None => Some(Found::Namespace(portions)),
                },
                Some(regular) => Some(regular),
                None => sandbox.find_top(last),
            }
        };
        Ok(match found {
            None => vm.ctx.none(),
            Some(Found::Module(origin)) => vm
                .new_tuple((
                    vm.ctx.new_str("module"),
                    vm.ctx.new_str(origin),
                    vm.ctx.none(),
                ))
                .into(),
            Some(Found::Package(origin, folder)) => vm
                .new_tuple((
                    vm.ctx.new_str("package"),
                    vm.ctx.new_str(origin),
                    strings(vm, vec![folder]),
                ))
                .into(),
            Some(Found::Namespace(folders)) => vm
                .new_tuple((
                    vm.ctx.new_str("namespace"),
                    vm.ctx.none(),
                    strings(vm, folders),
                ))
                .into(),
        })
    }

    #[pyfunction]
    fn source(origin: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<String> {
        let sandbox = sandbox(vm)?;
        let bytes = sandbox
            .files
            .read(origin.as_str())
            .map_err(|error| os_error(vm, &error))?;
        Ok(super::super::files::source_text(&bytes))
    }

    #[pyfunction]
    fn fs_read(path: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let sandbox = sandbox(vm)?;
        let bytes = sandbox
            .files
            .read(path.as_str())
            .map_err(|error| os_error(vm, &error))?;
        Ok(vm.ctx.new_bytes(bytes).into())
    }

    #[pyfunction]
    fn fs_write(
        path: PyUtf8StrRef,
        data: ArgBytesLike,
        append: bool,
        vm: &VirtualMachine,
    ) -> PyResult<()> {
        let sandbox = sandbox(vm)?;
        data.with_ref(|bytes| sandbox.files.write(path.as_str(), bytes, append))
            .map_err(|error| os_error(vm, &error))
    }

    #[pyfunction]
    fn fs_stat(path: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<PyTupleRef> {
        let sandbox = sandbox(vm)?;
        let stat = sandbox
            .files
            .stat(path.as_str())
            .map_err(|error| os_error(vm, &error))?;
        Ok(vm.new_tuple((stat.dir, stat.size, stat.modified)))
    }

    #[pyfunction]
    fn fs_list(path: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let sandbox = sandbox(vm)?;
        let names = sandbox
            .files
            .list(path.as_str())
            .map_err(|error| os_error(vm, &error))?;
        Ok(strings(vm, names))
    }

    #[pyfunction]
    fn fs_writable(path: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<bool> {
        Ok(sandbox(vm)?.files.is_writable(path.as_str()))
    }

    #[pyfunction]
    fn fs_mkdir(path: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<()> {
        sandbox(vm)?
            .files
            .mkdir(path.as_str())
            .map_err(|error| os_error(vm, &error))
    }

    #[pyfunction]
    fn fs_rmdir(path: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<()> {
        sandbox(vm)?
            .files
            .rmdir(path.as_str())
            .map_err(|error| os_error(vm, &error))
    }

    #[pyfunction]
    fn fs_remove(path: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<()> {
        sandbox(vm)?
            .files
            .remove(path.as_str())
            .map_err(|error| os_error(vm, &error))
    }

    #[pyfunction]
    fn fs_rename(from: PyUtf8StrRef, to: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<()> {
        sandbox(vm)?
            .files
            .rename(from.as_str(), to.as_str())
            .map_err(|error| os_error(vm, &error))
    }

    #[pyfunction]
    fn fetch(
        method: PyUtf8StrRef,
        url: PyUtf8StrRef,
        headers: PyObjectRef,
        body: PyObjectRef,
        timeout: f64,
        follow: bool,
        vm: &VirtualMachine,
    ) -> PyResult<PyTupleRef> {
        let sandbox = sandbox(vm)?;
        let headers: Vec<(String, String)> = vm.extract_elements_with(&headers, |pair| {
            let parts: Vec<String> =
                vm.extract_elements_with(&pair, |part| String::try_from_object(vm, part))?;
            match <[String; 2]>::try_from(parts) {
                Ok([name, value]) => Ok((name, value)),
                Err(_) => Err(vm.new_value_error("headers are (name, value) pairs")),
            }
        })?;
        let body = if vm.is_none(&body) {
            None
        } else {
            Some(
                ArgBytesLike::try_from_object(vm, body)?
                    .borrow_buf()
                    .to_vec(),
            )
        };
        let asked = if timeout.is_finite() && timeout > 0.0 {
            Duration::from_secs_f64(timeout.min(300.0))
        } else {
            Duration::from_secs(20)
        };
        let Some(allowed) = sandbox.time_left(asked) else {
            return Err(vm
                .new_os_subtype_error(vm.ctx.exceptions.timeout_error.to_owned(), None, TIMED_OUT)
                .upcast());
        };
        sandbox.note_request(url.as_str());
        match sandbox.fetcher.fetch(
            method.as_str(),
            url.as_str(),
            &headers,
            body,
            allowed,
            follow,
        ) {
            Ok(answer) => {
                let headers: Vec<PyObjectRef> = answer
                    .headers
                    .into_iter()
                    .map(|(name, value)| {
                        vm.new_tuple((vm.ctx.new_str(name), vm.ctx.new_str(value)))
                            .into()
                    })
                    .collect();
                Ok(vm.new_tuple((
                    answer.status,
                    vm.ctx.new_str(answer.url),
                    vm.ctx.new_list(headers),
                    vm.ctx.new_bytes(answer.body),
                )))
            }
            Err(FetchError::Timeout(message)) => Err(vm
                .new_os_subtype_error(vm.ctx.exceptions.timeout_error.to_owned(), None, message)
                .upcast()),
            Err(FetchError::Failed(message)) => Err(vm
                .new_os_subtype_error(vm.ctx.exceptions.connection_error.to_owned(), None, message)
                .upcast()),
        }
    }

    #[pyfunction]
    fn log(level: i64, message: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<()> {
        sandbox(vm)?.note(level, message.as_str());
        Ok(())
    }

    #[pyfunction]
    fn sleep(seconds: f64, vm: &VirtualMachine) -> PyResult<()> {
        let sandbox = sandbox(vm)?;
        let asked = if seconds.is_finite() && seconds > 0.0 {
            Duration::from_secs_f64(seconds.min(3600.0))
        } else {
            return Ok(());
        };
        let Some(allowed) = sandbox.time_left(asked) else {
            return Err(vm
                .new_os_subtype_error(vm.ctx.exceptions.timeout_error.to_owned(), None, TIMED_OUT)
                .upcast());
        };
        std::thread::sleep(allowed);
        if allowed < asked {
            return Err(vm
                .new_os_subtype_error(vm.ctx.exceptions.timeout_error.to_owned(), None, TIMED_OUT)
                .upcast());
        }
        Ok(())
    }

    #[pyfunction]
    fn urandom(size: usize, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
        let mut bytes = vec![0u8; size.min(1 << 20)];
        getrandom::fill(&mut bytes).map_err(|error| vm.new_os_error(error.to_string()))?;
        Ok(vm.ctx.new_bytes(bytes).into())
    }

    #[pyfunction]
    fn platform() -> &'static str {
        if cfg!(target_os = "android") {
            "android"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        }
    }

    #[pyfunction]
    fn machine() -> &'static str {
        match std::env::consts::ARCH {
            "aarch64" => "aarch64",
            "arm" => "armv7l",
            "x86" => "i686",
            other => other,
        }
    }
}

/// `posix`, which `os` builds on: filled from `lib/_raygun_posix.py` as it's first imported.
#[rustpython_vm::pymodule(name = "posix")]
pub(crate) mod posix {
    use rustpython_vm::builtins::PyModule;
    use rustpython_vm::compiler::Mode;
    use rustpython_vm::scope::Scope;
    use rustpython_vm::{Py, PyResult, VirtualMachine};

    const FILL: &str = "import _raygun_posix as _source\nfor _name, _value in vars(_source).items():\n    if not _name.startswith('__'):\n        globals()[_name] = _value\ndel _source, _name, _value\n";

    pub(crate) fn module_exec(vm: &VirtualMachine, module: &Py<PyModule>) -> PyResult<()> {
        __module_exec(vm, module);
        let code = vm
            .compile(FILL, Mode::Exec, "<posix>")
            .map_err(|error| error.into_pyexception(vm, Some(FILL)))?;
        vm.run_code_obj(code, Scope::with_builtins(None, module.dict(), vm))?;
        Ok(())
    }
}

/// `_raygun_sqlite`: `lib/sqlite3` over [`super::sqlite`]. Calls answer a tuple whose first item
/// is SQLite's error code (0 when it worked), so Python raises its own `sqlite3` errors.
#[rustpython_vm::pymodule(name = "_raygun_sqlite")]
pub(crate) mod raygun_sqlite {
    use rustpython_vm::builtins::{
        PyBytes, PyDict, PyFloat, PyInt, PyStr, PyTupleRef, PyUtf8StrRef,
    };
    use rustpython_vm::function::ArgBytesLike;
    use rustpython_vm::{AsObject, PyObjectRef, PyResult, TryFromObject, VirtualMachine};

    use super::super::sqlite::{Database, Parameters, SqlError, Value};
    use super::sandbox;

    fn failed(vm: &VirtualMachine, error: SqlError) -> PyTupleRef {
        vm.new_tuple((error.code, vm.ctx.new_str(error.message)))
    }

    fn value_of(vm: &VirtualMachine, object: PyObjectRef) -> PyResult<Value> {
        if vm.is_none(&object) {
            return Ok(Value::Null);
        }
        if object.class().is(vm.ctx.types.bool_type) {
            return Ok(Value::Integer(i64::from(object.is(&vm.ctx.true_value))));
        }
        if object.downcast_ref::<PyInt>().is_some() {
            return i64::try_from_object(vm, object)
                .map(Value::Integer)
                .map_err(|_| {
                    vm.new_overflow_error("Python int too large to convert to SQLite INTEGER")
                });
        }
        if let Some(number) = object.downcast_ref::<PyFloat>() {
            return Ok(Value::Real(number.to_f64()));
        }
        if let Some(text) = object.downcast_ref::<PyStr>() {
            return Ok(Value::Text(text.to_string_lossy().into_owned()));
        }
        if object.downcast_ref::<PyBytes>().is_some()
            || ArgBytesLike::try_from_object(vm, object.clone()).is_ok()
        {
            let bytes = ArgBytesLike::try_from_object(vm, object)?;
            return Ok(Value::Blob(bytes.borrow_buf().to_vec()));
        }
        Err(vm.new_type_error(format!(
            "Error binding parameter: type '{}' is not supported",
            object.class().name()
        )))
    }

    fn object_of(vm: &VirtualMachine, value: Value) -> PyObjectRef {
        match value {
            Value::Null => vm.ctx.none(),
            Value::Integer(number) => vm.ctx.new_int(number).into(),
            Value::Real(number) => vm.ctx.new_float(number).into(),
            Value::Text(text) => vm.ctx.new_str(text).into(),
            Value::Blob(bytes) => vm.ctx.new_bytes(bytes).into(),
        }
    }

    #[pyfunction]
    fn version() -> String {
        super::super::sqlite::version()
    }

    #[pyfunction]
    fn open(path: PyUtf8StrRef, timeout: f64, vm: &VirtualMachine) -> PyResult<PyTupleRef> {
        let sandbox = sandbox(vm)?;
        let path = path.as_str();
        let real = if path == ":memory:" {
            None
        } else {
            sandbox
                .files
                .database_path(path)
                .map_err(|error| super::os_error(vm, &error))?
        };
        let busy = (timeout.clamp(0.0, 30.0) * 1000.0) as i32;
        match Database::open(real.as_deref(), busy) {
            Ok(database) => Ok(vm.new_tuple((0, sandbox.keep_database(database)))),
            Err(error) => Ok(failed(vm, error)),
        }
    }

    #[pyfunction]
    fn close(handle: i64, vm: &VirtualMachine) -> PyResult<()> {
        sandbox(vm)?.drop_database(handle);
        Ok(())
    }

    #[pyfunction]
    fn execute(
        handle: i64,
        sql: PyUtf8StrRef,
        parameters: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyTupleRef> {
        let sandbox = sandbox(vm)?;
        let parameters = if let Some(dict) = parameters.downcast_ref::<PyDict>() {
            let mut named = Vec::new();
            for (key, value) in dict {
                let key = String::try_from_object(vm, key)?;
                named.push((key, value_of(vm, value)?));
            }
            Parameters::Named(named)
        } else {
            Parameters::Positional(
                vm.extract_elements_with(&parameters, |item| value_of(vm, item))?,
            )
        };
        let outcome = sandbox.with_database(handle, |database| {
            database.execute(sql.as_str(), &parameters)
        });
        let outcome = match outcome {
            None => {
                return Ok(
                    vm.new_tuple((21, vm.ctx.new_str("Cannot operate on a closed database.")))
                );
            }
            Some(Err(error)) => return Ok(failed(vm, error)),
            Some(Ok(outcome)) => outcome,
        };
        let columns: Vec<PyObjectRef> = outcome
            .columns
            .into_iter()
            .map(|(name, declared)| {
                let declared =
                    declared.map_or_else(|| vm.ctx.none(), |text| vm.ctx.new_str(text).into());
                vm.new_tuple((vm.ctx.new_str(name), declared)).into()
            })
            .collect();
        let rows: Vec<PyObjectRef> = outcome
            .rows
            .into_iter()
            .map(|row| {
                vm.ctx
                    .new_tuple(row.into_iter().map(|value| object_of(vm, value)).collect())
                    .into()
            })
            .collect();
        Ok(vm.new_tuple((
            0,
            vm.ctx.new_list(columns),
            vm.ctx.new_list(rows),
            outcome.changes,
            outcome.last_row,
        )))
    }

    #[pyfunction]
    fn script(handle: i64, sql: PyUtf8StrRef, vm: &VirtualMachine) -> PyResult<PyTupleRef> {
        let sandbox = sandbox(vm)?;
        match sandbox.with_database(handle, |database| database.script(sql.as_str())) {
            None => Ok(vm.new_tuple((21, vm.ctx.new_str("Cannot operate on a closed database.")))),
            Some(Err(error)) => Ok(failed(vm, error)),
            Some(Ok(())) => Ok(vm.new_tuple((0,))),
        }
    }

    #[pyfunction]
    fn autocommit(handle: i64, vm: &VirtualMachine) -> PyResult<bool> {
        Ok(sandbox(vm)?
            .with_database(handle, Database::autocommit)
            .unwrap_or(true))
    }

    #[pyfunction]
    fn total_changes(handle: i64, vm: &VirtualMachine) -> PyResult<i64> {
        Ok(sandbox(vm)?
            .with_database(handle, Database::total_changes)
            .unwrap_or_default())
    }
}

/// Puts the sandbox on `_raygun`, where the functions above find it.
pub(crate) fn attach(vm: &VirtualMachine, sandbox: Arc<Sandbox>) -> PyResult<()> {
    use rustpython_vm::PyPayload;
    let module = vm.import("_raygun", 0)?;
    let object: PyObjectRef = raygun::SandboxRef::new(sandbox).into_ref(&vm.ctx).into();
    module.set_attr("_sandbox", object, vm)
}
