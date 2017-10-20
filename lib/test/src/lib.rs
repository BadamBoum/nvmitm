#[macro_use]
extern crate log;
extern crate simplelog;
extern crate nvapi;
extern crate nvapi_sys as sys;
extern crate nvmitm;

use std::os::raw::c_void;
use std::ffi::CStr;
use nvmitm::Unsafe;
use nvapi::types::RawConversion;
use sys::{Api, NvAPI_Status};

const LOG_PATH: &'static str = "E:/nvlog.txt";
const NVAPI_PATH: &'static [u8] = b"E:/nvapi.dll\0";

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn nvapi_QueryInterface(id: u32) -> *const c_void {
    nvmitm::query_interface(id, || {
        let logger = ::std::fs::File::create(LOG_PATH).unwrap();
        simplelog::WriteLogger::init(simplelog::LogLevelFilter::Debug, Default::default(), logger).unwrap();

        unsafe {
            if let Some(ptr) = nvmitm::get_query_interface(CStr::from_bytes_with_nul(NVAPI_PATH).unwrap().as_ptr()) {
                sys::nvapi::set_query_interface(ptr);
            } else {
                error!("failed to resolve nvapi_QueryInterface");
            }
        }

        let mut config = nvmitm::Configuration::default();
        config.pre_hook = Some(pre_log);
        config.hooks.insert(Api::NvAPI_GPU_SetClockBoostTable, unsafe { Unsafe::new(hook_set_clock_boost_table as usize as *const _) });

        config
    })
}

pub fn pre_log(id: Result<Api, u32>) {
    match id {
        Ok(api) => info!("{:?}()", api),
        Err(id) => info!("Unknown API {} ()", id),
    }
}

pub extern "C" fn hook_set_clock_boost_table(gpu: sys::handles::NvPhysicalGpuHandle, table: &sys::gpu::clock::private::NV_CLOCK_TABLE) -> NvAPI_Status {
    info!("NvAPI_GPU_SetClockBoostTable({:?}, {:#?})", gpu, RawConversion::convert_raw(table).map_err(|_| table));

    unsafe {
        sys::gpu::clock::private::NvAPI_GPU_SetClockBoostTable(gpu, table)
    }
}
