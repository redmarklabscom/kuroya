use crate::path_display::sanitized_display_label_cow;
use anyhow::{Context, anyhow, bail};
use kuroya_core::{PluginCapabilities, PluginRuntimeRegistration, normalize_child_path};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};
use wasmi::{
    Caller, Config, Engine, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc, core::TrapCode,
};

const PLUGIN_COMMAND_WASM_MAX_BYTES: u64 = 8 * 1024 * 1024;
const PLUGIN_COMMAND_MODULE_CACHE_MAX_ENTRIES: usize = 16;
const PLUGIN_COMMAND_FUEL: u64 = 10_000_000;
const PLUGIN_COMMAND_MEMORY_MAX_BYTES: usize = 16 * 1024 * 1024;
const PLUGIN_COMMAND_TABLE_MAX_ELEMENTS: usize = 4096;
const PLUGIN_COMMAND_STATUS_MAX_BYTES: usize = 16 * 1024;
const PLUGIN_COMMAND_STATUS_MAX_CHARS: usize = 240;
const PLUGIN_COMMAND_MEMORY_EXPORT: &str = "memory";
pub(crate) const PLUGIN_COMMAND_DEFAULT_EXPORT: &str = "kuroya_plugin_command";

static PLUGIN_COMMAND_ENGINE: OnceLock<Engine> = OnceLock::new();
static PLUGIN_COMMAND_MODULE_CACHE: OnceLock<Mutex<PluginCommandModuleCache>> = OnceLock::new();

#[cfg(test)]
static PLUGIN_COMMAND_CACHED_MODULE_COMPILES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginCommandExecution {
    pub(crate) exit_code: i32,
    pub(crate) status: Option<String>,
    pub(crate) used_default_export: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PluginCommandModuleCacheStats {
    pub(crate) entries: usize,
    pub(crate) capacity: usize,
}

#[derive(Debug)]
struct PluginCommandHostState {
    status: Option<String>,
    limits: StoreLimits,
}

impl Default for PluginCommandHostState {
    fn default() -> Self {
        Self {
            status: None,
            limits: plugin_command_store_limits(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginCommandModuleCacheKey {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct PluginCommandModuleCacheEntry {
    key: PluginCommandModuleCacheKey,
    module: Module,
}

#[derive(Debug, Default)]
struct PluginCommandModuleCache {
    entries: VecDeque<PluginCommandModuleCacheEntry>,
}

impl PluginCommandModuleCache {
    fn get(&mut self, key: &PluginCommandModuleCacheKey) -> Option<Module> {
        let index = self.entries.iter().position(|entry| &entry.key == key)?;
        let entry = self.entries.remove(index)?;
        let module = entry.module.clone();
        self.entries.push_back(entry);
        Some(module)
    }

    fn insert(&mut self, key: PluginCommandModuleCacheKey, module: Module) {
        self.entries.retain(|entry| entry.key.path != key.path);
        while self.entries.len() >= PLUGIN_COMMAND_MODULE_CACHE_MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries
            .push_back(PluginCommandModuleCacheEntry { key, module });
    }
}

pub(crate) fn execute_plugin_command(
    runtime: &PluginRuntimeRegistration,
    command_id: &str,
) -> anyhow::Result<PluginCommandExecution> {
    validate_plugin_command_capabilities(&runtime.capabilities)?;
    let entry = plugin_entry_path(runtime)?;
    let module = cached_plugin_command_module(&entry)?;
    execute_plugin_command_module(&module, command_id)
}

fn validate_plugin_command_capabilities(capabilities: &PluginCapabilities) -> anyhow::Result<()> {
    if !capabilities.commands {
        bail!("plugin does not declare command capability");
    }

    let unsupported = unsupported_runtime_capabilities(capabilities);
    if !unsupported.is_empty() {
        bail!(
            "plugin declares unsupported runtime capabilities: {}",
            unsupported.join(", ")
        );
    }
    Ok(())
}

fn unsupported_runtime_capabilities(capabilities: &PluginCapabilities) -> Vec<&'static str> {
    let mut unsupported = Vec::new();
    if capabilities.workspace_read {
        unsupported.push("workspace_read");
    }
    if capabilities.workspace_write {
        unsupported.push("workspace_write");
    }
    if capabilities.process_spawn {
        unsupported.push("process_spawn");
    }
    if capabilities.network {
        unsupported.push("network");
    }
    unsupported
}

fn plugin_entry_path(runtime: &PluginRuntimeRegistration) -> anyhow::Result<PathBuf> {
    let Some(entry) = runtime.command_entry() else {
        bail!("plugin has no command entry");
    };
    normalize_child_path(&runtime.root, entry)
        .ok_or_else(|| anyhow!("plugin entry must stay inside the plugin root"))
}

fn cached_plugin_command_module(entry: &Path) -> anyhow::Result<Module> {
    let metadata = plugin_entry_metadata(entry)?;
    let key = plugin_command_module_cache_key(entry, &metadata);
    if let Some(module) = plugin_command_module_cache()
        .lock()
        .map_err(|_| anyhow!("plugin command module cache lock was poisoned"))?
        .get(&key)
    {
        return Ok(module);
    }

    let wasm = read_plugin_entry_bytes_with_metadata(entry, &metadata)?;
    let module = compile_plugin_command_module(&wasm)?;
    #[cfg(test)]
    PLUGIN_COMMAND_CACHED_MODULE_COMPILES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    plugin_command_module_cache()
        .lock()
        .map_err(|_| anyhow!("plugin command module cache lock was poisoned"))?
        .insert(key, module.clone());
    Ok(module)
}

fn plugin_command_module_cache_key(
    entry: &Path,
    metadata: &fs::Metadata,
) -> PluginCommandModuleCacheKey {
    PluginCommandModuleCacheKey {
        path: entry.to_path_buf(),
        len: metadata.len(),
        modified: metadata.modified().ok(),
    }
}

fn plugin_entry_metadata(entry: &Path) -> anyhow::Result<fs::Metadata> {
    let metadata = fs::metadata(entry).map_err(plugin_entry_read_error)?;
    if !metadata.is_file() {
        bail!("plugin entry is not a regular file");
    }
    if metadata.len() > PLUGIN_COMMAND_WASM_MAX_BYTES {
        bail!(
            "plugin entry exceeds the {} byte limit",
            PLUGIN_COMMAND_WASM_MAX_BYTES
        );
    }
    Ok(metadata)
}

fn read_plugin_entry_bytes_with_metadata(
    entry: &Path,
    metadata: &fs::Metadata,
) -> anyhow::Result<Vec<u8>> {
    let file = File::open(entry).map_err(plugin_entry_read_error)?;
    let mut limited = file.take(PLUGIN_COMMAND_WASM_MAX_BYTES.saturating_add(1));
    let capacity = usize::try_from(metadata.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    limited
        .read_to_end(&mut bytes)
        .map_err(plugin_entry_read_error)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > PLUGIN_COMMAND_WASM_MAX_BYTES {
        bail!(
            "plugin entry exceeds the {} byte limit",
            PLUGIN_COMMAND_WASM_MAX_BYTES
        );
    }
    Ok(bytes)
}

fn plugin_entry_read_error(error: std::io::Error) -> anyhow::Error {
    anyhow!("failed to read plugin entry: {error}")
}

fn plugin_command_engine() -> &'static Engine {
    PLUGIN_COMMAND_ENGINE.get_or_init(|| {
        let mut config = Config::default();
        config.consume_fuel(true);
        Engine::new(&config)
    })
}

fn plugin_command_module_cache() -> &'static Mutex<PluginCommandModuleCache> {
    PLUGIN_COMMAND_MODULE_CACHE.get_or_init(|| Mutex::new(PluginCommandModuleCache::default()))
}

pub(crate) fn plugin_command_module_cache_stats() -> PluginCommandModuleCacheStats {
    let entries = plugin_command_module_cache()
        .lock()
        .map(|cache| cache.entries.len())
        .unwrap_or_default();
    PluginCommandModuleCacheStats {
        entries,
        capacity: PLUGIN_COMMAND_MODULE_CACHE_MAX_ENTRIES,
    }
}

#[cfg(test)]
fn reset_plugin_command_module_cache_for_test() {
    if let Some(cache) = PLUGIN_COMMAND_MODULE_CACHE.get() {
        cache
            .lock()
            .expect("plugin command module cache should not be poisoned")
            .entries
            .clear();
    }
    PLUGIN_COMMAND_CACHED_MODULE_COMPILES.store(0, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
fn plugin_command_cached_module_compiles_for_test() -> usize {
    PLUGIN_COMMAND_CACHED_MODULE_COMPILES.load(std::sync::atomic::Ordering::SeqCst)
}

fn compile_plugin_command_module(wasm: &[u8]) -> anyhow::Result<Module> {
    Module::new(plugin_command_engine(), wasm).context("failed to load plugin wasm")
}

#[cfg(test)]
fn execute_plugin_command_wasm(
    wasm: &[u8],
    command_id: &str,
) -> anyhow::Result<PluginCommandExecution> {
    let module = compile_plugin_command_module(wasm)?;
    execute_plugin_command_module(&module, command_id)
}

fn execute_plugin_command_module(
    module: &Module,
    command_id: &str,
) -> anyhow::Result<PluginCommandExecution> {
    let engine = plugin_command_engine();
    let mut store = Store::new(engine, PluginCommandHostState::default());
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(PLUGIN_COMMAND_FUEL)
        .context("failed to initialize plugin fuel limit")?;
    let mut linker = Linker::new(engine);
    linker
        .func_wrap("kuroya", "status", plugin_status_host_call)
        .context("failed to install plugin host API")?;
    let instance = linker
        .instantiate(&mut store, module)
        .context("failed to instantiate plugin wasm")?
        .start(&mut store)
        .context("failed to start plugin wasm")?;
    let (command, used_default_export) = plugin_command_func(&instance, &store, command_id)?;
    let exit_code = command
        .call(&mut store, ())
        .map_err(plugin_command_call_error)?;

    let status = store.data().status.clone();
    Ok(PluginCommandExecution {
        exit_code,
        status,
        used_default_export,
    })
}

fn plugin_command_store_limits() -> StoreLimits {
    StoreLimitsBuilder::new()
        .memory_size(PLUGIN_COMMAND_MEMORY_MAX_BYTES)
        .table_elements(PLUGIN_COMMAND_TABLE_MAX_ELEMENTS)
        .instances(1)
        .memories(1)
        .tables(4)
        .trap_on_grow_failure(true)
        .build()
}

fn plugin_command_func(
    instance: &Instance,
    store: &Store<PluginCommandHostState>,
    command_id: &str,
) -> anyhow::Result<(TypedFunc<(), i32>, bool)> {
    if let Some(func) = instance.get_func(store, command_id) {
        return func
            .typed::<(), i32>(store)
            .map(|func| (func, false))
            .map_err(|error| {
                anyhow!(
                    "plugin command export {} must have signature () -> i32: {error}",
                    plugin_command_export_fragment(command_id)
                )
            });
    }

    if let Some(func) = instance.get_func(store, PLUGIN_COMMAND_DEFAULT_EXPORT) {
        return func
            .typed::<(), i32>(store)
            .map(|func| (func, true))
            .map_err(|error| {
                anyhow!(
                    "plugin default command export {} must have signature () -> i32: {error}",
                    PLUGIN_COMMAND_DEFAULT_EXPORT
                )
            });
    }

    bail!(
        "plugin command export {} was not found",
        plugin_command_export_fragment(command_id)
    )
}

fn plugin_command_call_error(error: wasmi::Error) -> anyhow::Error {
    if error.as_trap_code() == Some(TrapCode::OutOfFuel) {
        return anyhow!("plugin command exceeded the execution fuel limit");
    }
    anyhow!("plugin command trapped: {error}")
}

fn plugin_status_host_call(
    mut caller: Caller<'_, PluginCommandHostState>,
    ptr: i32,
    len: i32,
) -> Result<(), wasmi::Error> {
    let ptr = usize::try_from(ptr).map_err(|_| wasmi::Error::new("status pointer is negative"))?;
    let len = usize::try_from(len).map_err(|_| wasmi::Error::new("status length is negative"))?;
    if len > PLUGIN_COMMAND_STATUS_MAX_BYTES {
        return Err(wasmi::Error::new("status output exceeds host limit"));
    }
    if len == 0 {
        caller.data_mut().status = None;
        return Ok(());
    }

    let memory = caller
        .get_export(PLUGIN_COMMAND_MEMORY_EXPORT)
        .and_then(|export| export.into_memory())
        .ok_or_else(|| wasmi::Error::new("status output requires exported memory"))?;
    let mut bytes = vec![0; len];
    memory
        .read(&caller, ptr, &mut bytes)
        .map_err(|error| wasmi::Error::new(format!("failed to read status output: {error}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| wasmi::Error::new("status output must be valid UTF-8"))?;
    caller.data_mut().status = plugin_command_status_output(&text);
    Ok(())
}

pub(crate) fn plugin_command_status_output(value: &str) -> Option<String> {
    let output = sanitized_display_label_cow(value, PLUGIN_COMMAND_STATUS_MAX_CHARS, "");
    let output = output.trim();
    if output.is_empty() {
        None
    } else {
        Some(output.to_owned())
    }
}

fn plugin_command_export_fragment(value: &str) -> String {
    sanitized_display_label_cow(value, 96, "command").into_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        PLUGIN_COMMAND_DEFAULT_EXPORT, PLUGIN_COMMAND_MEMORY_MAX_BYTES,
        PLUGIN_COMMAND_WASM_MAX_BYTES, execute_plugin_command, execute_plugin_command_wasm,
        plugin_command_cached_module_compiles_for_test, plugin_command_status_output,
        reset_plugin_command_module_cache_for_test,
    };
    use kuroya_core::{
        PluginActivationEvent, PluginCapabilities, PluginRuntimeRegistration, normalize_child_path,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static CACHE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static TEST_PLUGIN_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn cache_test_lock() -> std::sync::MutexGuard<'static, ()> {
        CACHE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn execute_plugin_command_runs_named_export_and_reads_status() {
        let execution = execute_plugin_command_wasm(
            &wasm_bytes(
                r#"
                (module
                    (import "kuroya" "status" (func $status (param i32 i32)))
                    (memory (export "memory") 1)
                    (data (i32.const 0) "Hello from plugin")
                    (func (export "example.sayHello") (result i32)
                        i32.const 0
                        i32.const 17
                        call $status
                        i32.const 0
                    )
                )
                "#,
            ),
            "example.sayHello",
        )
        .expect("plugin command should run");

        assert_eq!(execution.exit_code, 0);
        assert_eq!(execution.status.as_deref(), Some("Hello from plugin"));
        assert!(!execution.used_default_export);
    }

    #[test]
    fn execute_plugin_command_uses_default_export_when_command_export_is_absent() {
        let execution = execute_plugin_command_wasm(
            &wasm_bytes(&format!(
                r#"
                (module
                    (func (export "{PLUGIN_COMMAND_DEFAULT_EXPORT}") (result i32)
                        i32.const 0
                    )
                )
                "#
            )),
            "example.missing",
        )
        .expect("default export should run");

        assert_eq!(execution.exit_code, 0);
        assert!(execution.status.is_none());
        assert!(execution.used_default_export);
    }

    #[test]
    fn execute_plugin_command_reports_nonzero_exit_without_runtime_error() {
        let execution = execute_plugin_command_wasm(
            &wasm_bytes(
                r#"
                (module
                    (func (export "example.fail") (result i32)
                        i32.const 7
                    )
                )
                "#,
            ),
            "example.fail",
        )
        .expect("nonzero command exit is a plugin result");

        assert_eq!(execution.exit_code, 7);
    }

    #[test]
    fn execute_plugin_command_rejects_missing_export() {
        let error = execute_plugin_command_wasm(
            &wasm_bytes(
                r#"
                (module
                    (func (export "other.command") (result i32)
                        i32.const 0
                    )
                )
                "#,
            ),
            "example.missing",
        )
        .expect_err("missing command export should fail");

        assert!(error.to_string().contains("was not found"));
    }

    #[test]
    fn execute_plugin_command_rejects_wrong_signature() {
        let error = execute_plugin_command_wasm(
            &wasm_bytes(
                r#"
                (module
                    (func (export "example.bad") (param i32) (result i32)
                        local.get 0
                    )
                )
                "#,
            ),
            "example.bad",
        )
        .expect_err("wrong command export signature should fail");

        assert!(error.to_string().contains("must have signature"));
    }

    #[test]
    fn execute_plugin_command_stops_infinite_loop_with_fuel() {
        let error = execute_plugin_command_wasm(
            &wasm_bytes(
                r#"
                (module
                    (func (export "example.loop") (result i32)
                        loop $again
                            br $again
                        end
                        i32.const 0
                    )
                )
                "#,
            ),
            "example.loop",
        )
        .expect_err("fuel should stop infinite loop");

        assert!(error.to_string().contains("fuel"));
    }

    #[test]
    fn execute_plugin_command_rejects_large_initial_memory() {
        let pages = PLUGIN_COMMAND_MEMORY_MAX_BYTES / 65_536 + 1;
        let error = execute_plugin_command_wasm(
            &wasm_bytes(&format!(
                r#"
                (module
                    (memory {pages})
                    (func (export "example.run") (result i32)
                        i32.const 0
                    )
                )
                "#
            )),
            "example.run",
        )
        .expect_err("initial memory should be bounded");

        assert!(error.to_string().contains("instantiate"));
    }

    #[test]
    fn execute_plugin_command_reuses_cached_module_for_repeated_entry() {
        let _guard = cache_test_lock();
        reset_plugin_command_module_cache_for_test();
        let temp = TestPluginDir::new();
        let wasm_path = temp.write_wasm(
            "plugin.wasm",
            r#"
            (module
                (func (export "example.run") (result i32)
                    i32.const 13
                )
            )
            "#,
        );
        let runtime = runtime_with_entry(temp.root(), wasm_path);

        let first = execute_plugin_command(&runtime, "example.run")
            .expect("first plugin command run should compile");
        let second = execute_plugin_command(&runtime, "example.run")
            .expect("second plugin command run should reuse the cached module");

        assert_eq!(first.exit_code, 13);
        assert_eq!(second.exit_code, 13);
        assert_eq!(plugin_command_cached_module_compiles_for_test(), 1);
    }

    #[test]
    fn execute_plugin_command_invalidates_cached_module_when_entry_changes() {
        let _guard = cache_test_lock();
        reset_plugin_command_module_cache_for_test();
        let temp = TestPluginDir::new();
        let wasm_path = temp.write_wasm(
            "plugin.wasm",
            r#"
            (module
                (func (export "example.run") (result i32)
                    i32.const 1
                )
            )
            "#,
        );
        let runtime = runtime_with_entry(temp.root(), wasm_path);

        let first = execute_plugin_command(&runtime, "example.run")
            .expect("first plugin command run should compile");
        temp.write_wasm(
            "plugin.wasm",
            r#"
            (module
                (func (export "example.helper") (result i32)
                    i32.const 0
                )
                (func (export "example.run") (result i32)
                    i32.const 2
                )
            )
            "#,
        );
        let second = execute_plugin_command(&runtime, "example.run")
            .expect("changed plugin command entry should recompile");

        assert_eq!(first.exit_code, 1);
        assert_eq!(second.exit_code, 2);
        assert_eq!(plugin_command_cached_module_compiles_for_test(), 2);
    }

    #[test]
    fn execute_plugin_command_rejects_unsupported_capabilities() {
        let temp = TestPluginDir::new();
        let wasm_path = temp.write_wasm("plugin.wasm", "(module)");
        let runtime = runtime_with_entry(temp.root(), wasm_path);
        let runtime = PluginRuntimeRegistration {
            capabilities: PluginCapabilities {
                commands: true,
                workspace_read: true,
                ..PluginCapabilities::default()
            },
            ..runtime
        };

        let error = execute_plugin_command(&runtime, "example.run")
            .expect_err("unsupported capability should fail closed");

        assert!(error.to_string().contains("workspace_read"));
    }

    #[test]
    fn execute_plugin_command_rejects_entry_outside_plugin_root() {
        let temp = TestPluginDir::new();
        let outside_temp = TestPluginDir::new();
        let outside = outside_temp.write_wasm("outside.wasm", "(module)");
        let runtime = runtime_with_entry(temp.root(), outside);

        let error =
            execute_plugin_command(&runtime, "example.run").expect_err("outside entry should fail");

        assert!(error.to_string().contains("plugin root"));
    }

    #[test]
    fn execute_plugin_command_rejects_oversized_entry_without_reading_unbounded() {
        let temp = TestPluginDir::new();
        let entry = temp.root().join("large.wasm");
        fs::write(
            &entry,
            vec![0_u8; usize::try_from(PLUGIN_COMMAND_WASM_MAX_BYTES).unwrap() + 1],
        )
        .expect("write oversized plugin entry");
        let runtime = runtime_with_entry(temp.root(), entry);

        let error = execute_plugin_command(&runtime, "example.run")
            .expect_err("oversized entry should fail");

        assert!(error.to_string().contains("byte limit"));
    }

    #[test]
    fn plugin_command_status_output_sanitizes_and_truncates() {
        let output = plugin_command_status_output(&format!("done\n{}\u{202e}", "x".repeat(512)))
            .expect("non-empty output");

        assert!(!output.chars().any(char::is_control));
        assert!(!output.contains('\u{202e}'));
        assert!(output.contains("..."));
        assert!(output.chars().count() <= 240);
        assert!(plugin_command_status_output(" \n \u{202e}").is_none());
    }

    fn wasm_bytes(wat: &str) -> Vec<u8> {
        wat::parse_str(wat).expect("test wat should compile")
    }

    fn runtime_with_entry(root: &Path, entry: PathBuf) -> PluginRuntimeRegistration {
        PluginRuntimeRegistration {
            plugin_id: "example.plugin".to_owned(),
            name: "Example".to_owned(),
            version: "0.1.0".to_owned(),
            root: root.to_path_buf(),
            entry: Some(entry),
            activation_events: vec![PluginActivationEvent::OnCommand("example.run".to_owned())],
            capabilities: PluginCapabilities {
                commands: true,
                ..PluginCapabilities::default()
            },
        }
    }

    struct TestPluginDir {
        root: PathBuf,
    }

    impl TestPluginDir {
        fn new() -> Self {
            let mut root = std::env::temp_dir();
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos();
            let counter = TEST_PLUGIN_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            root.push(format!(
                "kuroya-plugin-test-{}-{unique}-{counter}",
                std::process::id(),
            ));
            fs::create_dir_all(&root).expect("create temp plugin dir");
            Self { root }
        }

        fn root(&self) -> &Path {
            &self.root
        }

        fn write_wasm(&self, name: &str, wat: &str) -> PathBuf {
            let relative = Path::new(name);
            let path = normalize_child_path(&self.root, relative).expect("test child path");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create test wasm parent");
            }
            fs::write(&path, wasm_bytes(wat)).expect("write test wasm");
            path
        }
    }

    impl Drop for TestPluginDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
