//#![deny(missing_docs)]
#![doc(html_root_url = "http://arcnmx.github.io/nvmitm/")]

#[macro_use]
extern crate lazy_static;
#[cfg(windows)]
extern crate winapi;
#[cfg(all(windows, not(feature = "winapi3")))]
extern crate kernel32;
extern crate nvapi_sys as sys;

mod thunk;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::RwLock;
use std::os::raw::{c_void, c_char};
use std::{ptr, mem};
use thunk::Thunk;

pub use sys::Api;

#[cfg(all(windows, target_pointer_width = "32"))]
const THUNK_PRE: &'static [u8] = include_bytes!(concat!(env!("OUT_DIR"), "/thunk_pre_x86.bin"));
#[cfg(not(windows))]
const THUNK_PRE: &'static [u8] = &[];

lazy_static! {
    static ref CACHE: RwLock<Option<BTreeMap<u32, Func>>> = RwLock::new(None);
    static ref HOOKS: RwLock<BTreeMap<u32, Unsafe<*const c_void>>> = RwLock::new(Default::default());
}

static PRE_HOOK: AtomicUsize = ATOMIC_USIZE_INIT;

#[derive(Debug)]
enum Func {
    Pointer(Unsafe<*const c_void>),
    Thunk(Unsafe<Box<Thunk>>),
}

impl Func {
    fn ptr(&self) -> *const c_void {
        match *self {
            Func::Pointer(ref ptr) => *ptr.get(),
            Func::Thunk(ref thunk) => thunk.get().ptr(),
        }
    }
}

#[cfg(windows)]
pub unsafe fn get_query_interface(library_path: *const c_char) -> Option<sys::nvapi::QueryInterfaceFn> {
    #[cfg(feature = "winapi3")]
    use winapi::um::libloaderapi::{GetProcAddress, LoadLibraryA};
    #[cfg(not(feature = "winapi3"))]
    use kernel32::{GetProcAddress, LoadLibraryA};

    let lib = LoadLibraryA(library_path);

    if !lib.is_null() {
        let ptr = GetProcAddress(lib, sys::nvapi::FN_NAME.as_ptr() as *const _);
        if !ptr.is_null() {
            mem::transmute(ptr)
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(not(windows))]
pub unsafe fn get_query_interface(library_path: *const c_char) -> Option<sys::nvapi::QueryInterfaceFn> {
    unimplemented!()
}


/// Function prototype to be called before an NVAPI function
pub type PreHookFn = fn (Result<sys::Api, u32>);

/// Fields may be added over time - construct using `Default`.
#[derive(Debug, Default)]
pub struct Configuration {
    pub pre_hook: Option<PreHookFn>,
    pub hooks: BTreeMap<sys::Api, Unsafe<*const c_void>>,
}

/// A type that cannot be constructed without `unsafe` code. Used for wrapping
/// pointers in otherwise safe-accessible types. Contained value is marked as
/// both `Sync` and `Send`.
#[derive(Copy, Clone, Debug)]
pub struct Unsafe<T>(T);

unsafe impl<T> Sync for Unsafe<T> { }
unsafe impl<T> Send for Unsafe<T> { }

impl<T> Unsafe<T> {
    pub unsafe fn new(t: T) -> Self {
        Unsafe(t)
    }

    pub fn get(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

pub fn query_interface<F: FnOnce() -> Configuration>(id: u32, init: F) -> *const c_void {
    let cache = CACHE.read().unwrap();
    let cache = if cache.is_none() {
        drop(cache);

        let mut cache = CACHE.write().unwrap();
        if cache.is_none() {
            *cache = Some(Default::default());

            let config = init();

            if let Some(pre_hook) = config.pre_hook {
                PRE_HOOK.store(pre_hook as usize, Ordering::SeqCst);
            }

            let mut hooks = HOOKS.write().unwrap();
            for (api, hook) in config.hooks {
                hooks.insert(api.id(), hook);
            }
        }

        drop(cache);
        CACHE.read().unwrap()
    } else {
        cache
    };

    if let Some(ref cache) = *cache {
        match cache.get(&id) {
            Some(func) => return func.ptr(),
            None => (),
        }
    }

    drop(cache);
    let mut cache = CACHE.write().unwrap();
    if let Some(ref mut cache) = *cache {
        match generate_wrapper(id) {
            Ok(func) => {
                let ptr = func.ptr();
                cache.insert(id, func);
                ptr
            },
            Err(..) => ptr::null(),
        }
    } else {
        unreachable!()
    }
}

extern "C" fn thunk_fn(thunk: &'static Thunk) -> *const c_void {
    let pre_hook = PRE_HOOK.load(Ordering::SeqCst);
    if pre_hook != 0 {
        let pre_hook: PreHookFn = unsafe { mem::transmute(pre_hook) };
        let id = thunk.id;
        pre_hook(Api::from_id(id).map_err(|_| id));
    }

    thunk.nvapi
}

fn generate_wrapper(id: u32) -> sys::Result<Func> {
    match HOOKS.read().unwrap().get(&id) {
        Some(func) => Ok(Func::Pointer(unsafe { Unsafe::new(*func.get()) })),
        None => Ok(Func::Thunk(unsafe { Unsafe::new(Thunk::new(id, sys::nvapi_QueryInterface(id)? as *const _, THUNK_PRE, thunk_fn)) })),
    }
}
