#![no_std]

use core::panic::PanicInfo;

pub const FIXTURE_ID: &str = "adversarial/adversarial_case01_v1";
pub const FIXTURE_CATEGORY: &str = "adversarial";
pub const SOURCE_TEMPLATE_FAMILY: &str = "seeded_calls";

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
fn seed_vector_adversarial_adversarial_case01_v1() {
    unsafe {
        let _ = host_vec_new();
    }
}

#[inline(never)]
fn root_map_adversarial_adversarial_case01_v1() -> i64 {
    unsafe { host_map_new() }
}

#[no_mangle]
pub extern "C" fn entry_adversarial_case01_v1() -> i64 {
    seed_vector_adversarial_adversarial_case01_v1();
    root_map_adversarial_adversarial_case01_v1()
}
