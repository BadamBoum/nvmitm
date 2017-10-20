use std::os::raw::c_void;

pub type ThunkFn = extern "C" fn (thunk: &'static Thunk) -> *const c_void;

#[derive(Debug)]
#[repr(C)]
pub struct Thunk {
    pub callback: *const c_void,
    pub id: u32,
    pub nvapi: *const c_void,
    pub thunk: Box<[u8]>,
}

impl Thunk {
    pub fn ptr(&self) -> *const c_void {
        self.thunk.as_ptr() as *const _
    }

    pub fn new(id: u32, nvapi: *const c_void, thunk_data: &[u8], thunk_fn: ThunkFn) -> Box<Self> {
        let mut thunk = Box::new(Thunk {
            callback: thunk_fn as usize as *const c_void,
            id: id,
            nvapi: nvapi,
            thunk: {
                let mut v = Vec::new();
                v.extend(thunk_data);
                v.into_boxed_slice()
            },
        });

        #[cfg(target_pointer_width = "32")]
        fn address_thunk(thunk: &mut [u8], ptr: usize) {
            thunk[1] = ptr as u8;
            thunk[2] = (ptr >> 8) as u8;
            thunk[3] = (ptr >> 16) as u8;
            thunk[4] = (ptr >> 24) as u8;
        }

        #[cfg(target_pointer_width = "64")]
        fn address_thunk(thunk: &mut [u8], ptr: usize) {
            unimplemented!()
        }

        #[cfg(windows)]
        fn exec_thunk(thunk: &mut [u8]) {
            #[cfg(feature = "winapi3")]
            use winapi::um::memoryapi::VirtualProtect;
            #[cfg(feature = "winapi3")]
            use winapi::um::winnt::PAGE_EXECUTE_READWRITE;
            #[cfg(not(feature = "winapi3"))]
            use kernel32::VirtualProtect;
            #[cfg(not(feature = "winapi3"))]
            use winapi::winnt::PAGE_EXECUTE_READWRITE;

            let mut old = 0;
            unsafe { VirtualProtect(thunk.as_ptr() as *mut _, thunk.len() as _, PAGE_EXECUTE_READWRITE, &mut old) };
        }

        #[cfg(not(windows))]
        fn exec_thunk(thunk: &mut [u8]) {
            unimplemented!()
        }

        let ptr = &*thunk as *const _ as usize;
        address_thunk(&mut thunk.thunk, ptr);
        exec_thunk(&mut thunk.thunk);

        thunk
    }
}
