#![no_std]

use core::panic::PanicInfo;

pub const FIXTURE_ID: &str = "real_world/escrow_service_v1";
pub const FIXTURE_CATEGORY: &str = "real_world";
pub const SOURCE_TEMPLATE_FAMILY: &str = "grouped_helpers";

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
fn build_vector_real_world_escrow_service_v1() -> i64 {
    unsafe { host_vec_new() }
}

#[inline(never)]
fn build_map_real_world_escrow_service_v1() -> i64 {
    unsafe { host_map_new() }
}

#[inline(never)]
fn compose_state_real_world_escrow_service_v1() -> i64 {
    let _ = build_vector_real_world_escrow_service_v1();
    build_map_real_world_escrow_service_v1()
}

#[no_mangle]
pub extern "C" fn entry_escrow_service_v1() -> i64 {
    compose_state_real_world_escrow_service_v1()
}
