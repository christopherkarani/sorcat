#![no_std]

use core::panic::PanicInfo;

pub const FIXTURE_ID: &str = "real_world/token_swap_v1";
pub const FIXTURE_CATEGORY: &str = "real_world";
pub const SOURCE_TEMPLATE_FAMILY: &str = "tail_return";

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[link(wasm_import_module = "m")]
extern "C" {
    #[link_name = "_"]
    fn host_map_new() -> i64;
}

#[link(wasm_import_module = "v")]
extern "C" {
    #[link_name = "_"]
    fn host_vec_new() -> i64;
}

#[inline(never)]
fn terminal_map_real_world_token_swap_v1() -> i64 {
    unsafe { host_map_new() }
}

#[no_mangle]
pub extern "C" fn entry_token_swap_v1() -> i64 {
    unsafe {
        let _ = host_vec_new();
    }
    terminal_map_real_world_token_swap_v1()
}
