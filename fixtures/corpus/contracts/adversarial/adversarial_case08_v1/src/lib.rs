#![no_std]

use core::panic::PanicInfo;

pub const FIXTURE_ID: &str = "adversarial/adversarial_case08_v1";
pub const FIXTURE_CATEGORY: &str = "adversarial";
pub const SOURCE_TEMPLATE_FAMILY: &str = "module_bridge";

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
fn bridge_state_adversarial_adversarial_case08_v1() -> i64 {
    unsafe {
        let _ = host_vec_new();
        host_map_new()
    }
}

#[no_mangle]
pub extern "C" fn entry_adversarial_case08_v1() -> i64 {
    bridge_state_adversarial_adversarial_case08_v1()
}
