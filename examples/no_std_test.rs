#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use luminal::Runtime;

// This example demonstrates using Luminal in a no_std environment
// Note: This requires the alloc crate for heap allocation

#[no_mangle]
pub extern "C" fn no_std_test() -> i32 {
    let rt = Runtime::new().unwrap();

    let handle = rt.spawn(async {
        let mut results = Vec::new();
        for i in 0..10 {
            results.push(i * 2);
        }
        results.iter().sum::<i32>()
    });

    rt.block_on(handle)
}
#[cfg(not(feature = "std"))]
// Dummy panic handler for no_std
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}