# Luminal no_std Guide

This guide covers using Luminal in `no_std` environments, such as embedded systems and other constrained platforms.

## Overview

Luminal provides full async runtime support for `no_std` environments with the following characteristics:

- ✅ Single-threaded execution model (no threading support)
- ✅ Requires `alloc` for heap allocation
- ✅ Uses heapless data structures with bounded queues
- ✅ Explicit runtime context (no global functions)
- ✅ DLL boundary safe (no thread-local storage)
- ✅ Minimal resource usage
- ✅ Same async/await API as std version

## Installation

Add Luminal to your `Cargo.toml` with `default-features = false`:

```toml
[dependencies]
luminal = { version = "0.3.0", default-features = false }
```

## Requirements

### Rust Setup

Your `no_std` project needs:

```rust
#![no_std]
extern crate alloc;
```

### Dependencies

The `no_std` version of Luminal uses:
- `heapless` for bounded data structures
- `parking_lot` for synchronization primitives (no_std compatible)
- `crossbeam-deque` for work stealing (no threading in no_std mode)

## Basic Usage

### Creating and Using a Runtime

```rust
#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use luminal::Runtime;

fn main() -> Result<(), luminal::RuntimeError> {
    // Create a new runtime
    let rt = Runtime::new()?;

    // Spawn a task
    let handle = rt.spawn(async {
        let mut results = Vec::new();
        for i in 0..10 {
            results.push(i * 2);
        }
        results.iter().sum::<i32>()
    });

    // Block until the task completes
    let result = rt.block_on(handle);
    assert_eq!(result, 90);

    Ok(())
}
```

### Async Functions

```rust
#![no_std]
extern crate alloc;

use alloc::{vec::Vec, format};
use luminal::Runtime;

async fn process_data(data: Vec<u32>) -> Vec<u32> {
    // Simulate async processing
    data.iter().map(|x| x * 2).collect()
}

async fn complex_task() -> u32 {
    let data = alloc::vec![1, 2, 3, 4, 5];
    let processed = process_data(data).await;
    processed.iter().sum()
}

fn main() {
    let rt = Runtime::new().unwrap();
    let result = rt.block_on(complex_task());
    assert_eq!(result, 30); // (1+2+3+4+5) * 2 = 30
}
```

## Configuration and Limits

### Bounded Queues

The no_std version uses bounded queues with fixed capacities:

- **Task Queue**: 1024 tasks maximum
- **Result Queue**: 16 results per JoinHandle

These limits are compile-time constants to ensure deterministic memory usage.

### Memory Usage

The runtime uses a fixed amount of memory:
- Task queue: ~1MB (1024 tasks * ~1KB per task)
- Minimal overhead for synchronization
- No dynamic thread creation

## Architecture Differences

### std vs no_std

| Feature | std | no_std |
|---------|-----|---------|
| Threading | Multi-threaded work stealing | Single-threaded |
| Global Functions | `spawn()`, `block_on()` | None |
| Queue Type | Unbounded channels | Bounded heapless queues |
| Memory Model | Dynamic allocation | Fixed-size allocation |
| Synchronization | Standard library primitives | parking_lot no_std |

### Execution Model

In no_std mode, Luminal uses a simplified execution model:

1. **Single Event Loop**: All tasks run on the calling thread
2. **Cooperative Scheduling**: Tasks yield control at `.await` points
3. **Bounded Queues**: Prevents unbounded memory growth
4. **Busy Waiting**: Simple polling loop (no thread sleeping)

## Error Handling

```rust
#![no_std]
extern crate alloc;

use luminal::{Runtime, RuntimeError};

fn main() -> Result<(), RuntimeError> {
    let rt = Runtime::new()?;

    let handle = rt.spawn(async {
        // This task might fail
        if some_condition() {
            panic!("Task failed!");
        }
        42
    });

    // Handle potential task panics
    match rt.block_on(handle) {
        result => {
            // Task completed successfully
            println!("Result: {}", result);
            Ok(())
        }
    }
}

fn some_condition() -> bool { false }
```

## Best Practices

### 1. Minimize Task Queue Usage

Since the task queue is bounded, avoid creating too many concurrent tasks:

```rust
// Good: Process items in batches
async fn process_batch(items: &[u32]) -> Vec<u32> {
    items.iter().map(|x| x * 2).collect()
}

// Avoid: Creating many individual tasks
// for item in large_vec {
//     rt.spawn(async move { process_item(item) });
// }
```

### 2. Use Explicit Runtime References

Always pass runtime references explicitly:

```rust
async fn worker_task(rt: &Runtime, data: u32) -> u32 {
    let subtask = rt.spawn(async move { data * 2 });
    rt.block_on(subtask)
}
```

### 3. Handle Queue Capacity

Be aware of bounded queue limitations:

```rust
let rt = Runtime::new().unwrap();

// Check queue stats
let (queue_len, processed) = rt.stats();
if queue_len > 900 {  // Near capacity
    // Wait for tasks to complete or handle overflow
}
```

## Platform-Specific Considerations

### Embedded Targets

For embedded platforms like ARM Cortex-M:

```toml
# Cargo.toml
[target.'cfg(target_arch = "arm")'.dependencies]
luminal = { version = "0.3.0", default-features = false }

# Optional: Reduce memory usage
[features]
reduced-memory = []
```

### WASM

For WebAssembly targets:

```toml
# Cargo.toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
luminal = { version = "0.3.0", default-features = false }
```

## Debugging and Profiling

### Runtime Statistics

Monitor runtime performance:

```rust
let rt = Runtime::new().unwrap();

// ... spawn tasks ...

let (queue_len, tasks_processed) = rt.stats();
println!("Queue length: {}, Tasks processed: {}", queue_len, tasks_processed);
```

### Memory Usage

Track memory usage in constrained environments:

```rust
#![no_std]
extern crate alloc;

use alloc::alloc::{GlobalAlloc, Layout};
use luminal::Runtime;

// Custom allocator for tracking
struct TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Track allocations
        std::alloc::System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        std::alloc::System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;
```

## Examples

### LED Blinking (Embedded)

```rust
#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec::Vec;
use luminal::Runtime;

// Embedded HAL imports would go here
// use embedded_hal::digital::v2::OutputPin;

#[no_mangle]
pub extern "C" fn main() -> ! {
    let rt = Runtime::new().unwrap();

    let blink_task = rt.spawn(async {
        let mut counter = 0u32;
        loop {
            // Toggle LED (pseudocode)
            // led.toggle();
            counter += 1;

            // Yield to other tasks
            async_yield().await;

            if counter > 1000 {
                break;
            }
        }
        counter
    });

    let _result = rt.block_on(blink_task);

    // Embedded systems typically run forever
    loop {}
}

async fn async_yield() {
    // Minimal yield implementation
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
```

### Sensor Data Processing

```rust
#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use luminal::Runtime;

struct SensorReading {
    temperature: f32,
    humidity: f32,
}

async fn read_sensor() -> SensorReading {
    // Simulate sensor reading
    SensorReading {
        temperature: 25.0,
        humidity: 60.0,
    }
}

async fn process_readings(count: usize) -> f32 {
    let mut readings = Vec::new();

    for _ in 0..count {
        let reading = read_sensor().await;
        readings.push(reading.temperature);
    }

    readings.iter().sum::<f32>() / readings.len() as f32
}

fn main() {
    let rt = Runtime::new().unwrap();

    let avg_temp = rt.block_on(process_readings(10));

    // Use average temperature for control logic
    if avg_temp > 30.0 {
        // Turn on cooling
    }
}
```

## Migration from std

When migrating from std to no_std:

1. **Remove global functions**: Replace `spawn()` and `block_on()` with runtime methods
2. **Add bounds checking**: Handle queue capacity limits
3. **Replace std types**: Use `alloc` equivalents (`Vec`, `String`, etc.)
4. **Single-threaded mindset**: Design for cooperative multitasking

### Before (std):
```rust
use luminal::{spawn, block_on};

fn main() {
    let handle = spawn(async { 42 });
    let result = block_on(handle);
}
```

### After (no_std):
```rust
#![no_std]
extern crate alloc;

use luminal::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    let handle = rt.spawn(async { 42 });
    let result = rt.block_on(handle);
}
```

## Troubleshooting

### Common Issues

**Queue Full Errors**: Reduce concurrent tasks or increase processing speed
**Memory Exhaustion**: Monitor allocations and use streaming processing
**Blocking Operations**: Ensure all operations are truly async

### Performance Tips

1. **Batch Processing**: Group operations to reduce task overhead
2. **Bounded Resources**: Use fixed-size collections where possible
3. **Cooperative Yields**: Add manual yield points in long-running tasks
4. **Memory Reuse**: Reuse allocations instead of creating new ones

## Contributing

When contributing to no_std support:

1. Test on actual embedded targets
2. Measure memory usage
3. Ensure deterministic behavior
4. Document resource requirements

## Further Reading

- [Rust Embedded Book](https://doc.rust-lang.org/embedded-book/)
- [heapless documentation](https://docs.rs/heapless/)
- [parking_lot no_std guide](https://docs.rs/parking_lot/)
- [Async programming in embedded Rust](https://book.embassy.dev/)