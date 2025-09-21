use luminal::{Runtime, spawn, block_on};

// This example demonstrates using Luminal in a std environment
// Note: spawn and block_on global functions are only available with std feature

fn main() {
    println!("Testing Luminal with std features");

    // Using explicit runtime
    let rt = Runtime::new().unwrap();

    let handle = rt.spawn(async {
        println!("Task running on explicit runtime");
        42
    });

    let result = rt.block_on(handle);
    println!("Explicit runtime result: {}", result);

    // Using global functions (only available with std feature)
    let handle = spawn(async {
        println!("Task running on global runtime");
        100
    });

    let result = block_on(handle);
    println!("Global runtime result: {}", result);
}