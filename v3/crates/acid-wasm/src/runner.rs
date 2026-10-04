//! Runs one cart: load, version check, `acid_on_create`, `acid_redraw`, then
//! the host-owned event loop until close (spec §15.3), with each callback on
//! a fresh fuel budget and linear memory capped (§15.2).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;

use acid_api::{AcidApi, PolledEvent};
use wasmi::errors::{ErrorKind, InstantiationError, MemoryError, TableError};
use wasmi::{Config, Engine, Error, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TrapCode, TypedFunc, WasmParams, WasmResults};

use crate::{ABI_VERSION, CartEnd, WASM_MAX_TABLE_ELEMENTS, WASM_MAX_TABLES, WasmLimits, abi};

/// Default wait between idles when the cart doesn't export
/// `acid_poll_timeout_ms` (§15.3).
const DEFAULT_POLL_MS: i32 = 200;

/// Upper bound on a cart's poll timeout. The host thread blocks in
/// `poll_event` for this long, so a cart asking for `i32::MAX` (24 days)
/// would leave its loop unresponsive to everything but events; a minute
/// keeps idle ticks coming without busy-waiting.
const MAX_POLL_MS: i32 = 60_000;

/// Bytes in one wasm page.
const PAGE: usize = 64 * 1024;

/// Event kinds passed to `acid_on_event` (§15.3).
const EV_TOUCH: i32 = 1;
const EV_KEY: i32 = 2;
const EV_MOVED: i32 = 3;
const EV_RESIZED: i32 = 5;
const EV_CLOSE: i32 = 4;

/// Host state in the store: what imports reach, and the memory limiter.
pub(crate) struct Host {
    pub(crate) api: Arc<dyn AcidApi>,
    limits: WasmLimits,
    limiter: StoreLimits,
}

type Call0 = TypedFunc<(), ()>;

/// The cart's exports, resolved and type-checked once before anything runs.
struct Exports {
    version: TypedFunc<(), i32>,
    on_create: Call0,
    on_event: TypedFunc<(i32, i32, i32, i32), ()>,
    on_idle: Call0,
    redraw: Call0,
    poll_timeout_ms: Option<TypedFunc<(), i32>>,
    on_destroy: Option<Call0>,
}

impl Exports {
    fn resolve(store: &Store<Host>, inst: &Instance) -> Result<Self, String> {
        if inst.get_memory(store, "memory").is_none() {
            return Err("missing export `memory`".to_string());
        }
        Ok(Self {
            version: required(store, inst, "acid_abi_version")?,
            on_create: required(store, inst, "acid_on_create")?,
            on_event: required(store, inst, "acid_on_event")?,
            on_idle: required(store, inst, "acid_on_idle")?,
            redraw: required(store, inst, "acid_redraw")?,
            poll_timeout_ms: optional(store, inst, "acid_poll_timeout_ms")?,
            on_destroy: optional(store, inst, "acid_on_destroy")?,
        })
    }
}

fn required<P: WasmParams, R: WasmResults>(store: &Store<Host>, inst: &Instance, name: &str) -> Result<TypedFunc<P, R>, String> {
    optional(store, inst, name)?.ok_or_else(|| format!("missing export `{name}`"))
}

/// An optional export may be absent, but if present its signature must match.
fn optional<P: WasmParams, R: WasmResults>(store: &Store<Host>, inst: &Instance, name: &str) -> Result<Option<TypedFunc<P, R>>, String> {
    match inst.get_func(store, name) {
        None => Ok(None),
        Some(f) => f.typed(store).map(Some).map_err(|e| format!("export `{name}` has the wrong signature: {e}")),
    }
}

/// True for the limiter refusing a memory or table: at instantiation, or a
/// `memory.grow`/`table.grow` past the cap, which `trap_on_grow_failure`
/// turns into the `GrowthOperationLimited` trap.
fn denied_allocation(e: &Error) -> bool {
    e.as_trap_code() == Some(TrapCode::GrowthOperationLimited)
        || matches!(
            e.kind(),
            ErrorKind::Instantiation(InstantiationError::FailedToInstantiateMemory(MemoryError::ResourceLimiterDeniedAllocation))
                | ErrorKind::Memory(MemoryError::ResourceLimiterDeniedAllocation)
                | ErrorKind::Instantiation(InstantiationError::FailedToInstantiateTable(TableError::ResourceLimiterDeniedAllocation))
                | ErrorKind::Table(TableError::ResourceLimiterDeniedAllocation)
        )
}

/// Maps a failed call: fuel exhaustion is "stopped responding", growth past
/// the cap is out of memory (§15.2); anything else is a trap carrying its
/// message.
fn call_error(e: Error) -> CartEnd {
    if e.as_trap_code() == Some(TrapCode::OutOfFuel) {
        CartEnd::StoppedResponding
    } else if denied_allocation(&e) {
        CartEnd::OutOfMemory
    } else {
        CartEnd::Trap(e.to_string())
    }
}

/// One callback on a fresh fuel budget (§15.2).
fn call<P: WasmParams, R: WasmResults>(store: &mut Store<Host>, f: &TypedFunc<P, R>, params: P) -> Result<R, CartEnd> {
    let fuel = store.data().limits.fuel;
    store.set_fuel(fuel).map_err(call_error)?;
    f.call(&mut *store, params).map_err(call_error)
}

/// Instantiation failures: a declared memory or table over the cap is out of
/// memory, a trap in a start function ends the cart like any callback, and
/// anything else (an unknown import, a type mismatch, a second table) means
/// the module is refused.
fn instantiate_error(e: Error) -> CartEnd {
    if denied_allocation(&e) || e.as_trap_code().is_some() {
        call_error(e)
    } else {
        CartEnd::Refused(format!("cannot instantiate: {e}"))
    }
}

/// Runs one cart to completion on the current thread: load, version check,
/// on_create, redraw, then the event loop until close.
pub fn run_cart(api: Arc<dyn AcidApi>, module_bytes: &[u8], limits: WasmLimits) -> CartEnd {
    match run(api, module_bytes, limits) {
        Ok(()) => CartEnd::Closed,
        Err(end) => end,
    }
}

fn run(api: Arc<dyn AcidApi>, module_bytes: &[u8], limits: WasmLimits) -> Result<(), CartEnd> {
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, module_bytes).map_err(|e| CartEnd::Refused(format!("not a valid module: {e}")))?;

    // The caps cover the whole store (§15.2). Tables are capped as well as
    // memory: their elements live in host memory outside the linear-memory
    // cap. One instance, one memory and one table are all a cart needs, so
    // asking for more fails instantiation too.
    let max_bytes = (limits.max_pages as usize).saturating_mul(PAGE);
    let limiter = StoreLimitsBuilder::new()
        .memory_size(max_bytes)
        .table_elements(WASM_MAX_TABLE_ELEMENTS as usize)
        .instances(1)
        .memories(1)
        .tables(WASM_MAX_TABLES as usize)
        // Growth past a cap ends the cart (§15.2) instead of handing the
        // cart a -1 it might ignore.
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(&engine, Host { api: api.clone(), limits, limiter });
    store.limiter(|host| &mut host.limiter);

    let mut linker = Linker::new(&engine);
    abi::link(&mut linker).map_err(|e| CartEnd::Refused(format!("host import table: {e}")))?;

    // A start function runs during instantiation, so it gets a budget too.
    store.set_fuel(limits.fuel).map_err(call_error)?;
    let instance = linker.instantiate_and_start(&mut store, &module).map_err(instantiate_error)?;
    let ex = Exports::resolve(&store, &instance).map_err(CartEnd::Refused)?;

    let version = call(&mut store, &ex.version, ())?;
    if version != ABI_VERSION {
        return Err(CartEnd::Refused(format!("ABI version {version}, expected {ABI_VERSION}")));
    }

    call(&mut store, &ex.on_create, ())?;
    call(&mut store, &ex.redraw, ())?;

    loop {
        let timeout = match &ex.poll_timeout_ms {
            Some(f) => call(&mut store, f, ())?.clamp(1, MAX_POLL_MS),
            None => DEFAULT_POLL_MS,
        };
        match api.poll_event(i64::from(timeout)) {
            None => call(&mut store, &ex.on_idle, ())?,
            Some(PolledEvent::Touch { x, y, pressed }) => call(&mut store, &ex.on_event, (EV_TOUCH, x, y, i32::from(pressed)))?,
            Some(PolledEvent::Key { code, pressed }) => call(&mut store, &ex.on_event, (EV_KEY, code, i32::from(pressed), 0))?,
            Some(PolledEvent::Moved) => {
                call(&mut store, &ex.on_event, (EV_MOVED, 0, 0, 0))?;
                call(&mut store, &ex.redraw, ())?;
            }
            Some(PolledEvent::Resized { w, h }) => {
                call(&mut store, &ex.on_event, (EV_RESIZED, w, h, 0))?;
                call(&mut store, &ex.redraw, ())?;
            }
            Some(PolledEvent::Close) => {
                call(&mut store, &ex.on_event, (EV_CLOSE, 0, 0, 0))?;
                if let Some(f) = &ex.on_destroy {
                    call(&mut store, f, ())?;
                }
                return Ok(());
            }
        }
    }
}
