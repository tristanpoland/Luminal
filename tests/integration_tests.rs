use luminal::Runtime;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn test_basic_spawn_and_await() {
    let rt = Runtime::new().unwrap();
    let rt_clone = rt.clone();
    let result = rt.block_on(async move {
        let handle = rt_clone.spawn(async { 42 });
        handle.await
    });
    assert_eq!(result, 42);
}

#[test]
fn test_multiple_tasks() {
    let rt = Runtime::new().unwrap();
    let rt_clone = rt.clone();
    let result = rt.block_on(async move {
        let handle1 = rt_clone.spawn(async { 10 });
        let handle2 = rt_clone.spawn(async { 20 });
        let handle3 = rt_clone.spawn(async { 30 });
        
        let a = handle1.await;
        let b = handle2.await;
        let c = handle3.await;
        
        a + b + c
    });
    assert_eq!(result, 60);
}

#[test]
fn test_nested_spawns() {
    let rt = Runtime::new().unwrap();
    let rt_clone = rt.clone();
    let result = rt.block_on(async move {
        let outer = rt_clone.spawn(async {
            let rt2 = Runtime::new().unwrap();
            let inner = rt2.spawn(async { 5 });
            rt2.block_on(async { inner.await * 2 })
        });
        outer.await
    });
    assert_eq!(result, 10);
}

#[test]
fn test_runtime_new() {
    let rt = Runtime::new().unwrap();
    let handle = rt.spawn(async { 100 });
    let result = rt.block_on(async move {
        handle.await
    });
    assert_eq!(result, 100);
}

#[test]
fn test_shared_state() {
    let rt = Runtime::new().unwrap();
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();
    let rt_clone = rt.clone();
    
    rt.block_on(async move {
        let mut handles = Vec::new();
        
        for _ in 0..10 {
            let counter = counter_clone.clone();
            let handle = rt_clone.spawn(async move {
                let mut val = counter.lock().unwrap();
                *val += 1;
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await;
        }
    });
    
    assert_eq!(*counter.lock().unwrap(), 10);
}

#[test]
fn test_error_handling() {
    let result = std::panic::catch_unwind(|| {
        let rt = Runtime::new().unwrap();
        let rt_clone = rt.clone();
        rt.block_on(async move {
            rt_clone.spawn(async {
                panic!("test panic");
            }).await
        })
    });
    assert!(result.is_err());
}

#[test]
fn test_concurrent_runtimes() {
    let handles: Vec<_> = (0..4).map(|i| {
        thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            let rt_clone = rt.clone();
            rt.block_on(async move {
                let handle = rt_clone.spawn(async move { i * 2 });
                handle.await
            })
        })
    }).collect();
    
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results, vec![0, 2, 4, 6]);
}

#[test]
fn test_runtime_handle() {
    let rt = Runtime::new().unwrap();
    let handle = rt.handle();
    
    // Test that we can create a handle and spawn tasks
    let task_handle = handle.spawn(async { 777 });
    
    // Use the handle's runtime to wait for the result
    let result = handle.block_on(async move {
        task_handle.await
    });
    
    assert_eq!(result, 777);
}

#[tokio::test]
async fn test_inside_tokio_runtime() {
    // Test that Luminal can run inside a tokio runtime
    let result = tokio::task::spawn_blocking(|| {
        let rt = Runtime::new().unwrap();
        let rt_clone = rt.clone();
        rt.block_on(async move {
            let handle = rt_clone.spawn(async { 999 });
            handle.await
        })
    }).await.unwrap();
    
    assert_eq!(result, 999);
}

#[tokio::test]
async fn test_luminal_and_tokio_interop() {
    // Test spawning tokio tasks from within Luminal
    let tokio_result = tokio::spawn(async { 42 }).await.unwrap();
    
    let luminal_result = tokio::task::spawn_blocking(move || {
        let rt = Runtime::new().unwrap();
        let rt_clone = rt.clone();
        rt.block_on(async move {
            rt_clone.spawn(async move { tokio_result * 2 }).await
        })
    }).await.unwrap();
    
    assert_eq!(luminal_result, 84);
}

#[test]
fn test_dll_boundary_simulation() {
    // Simulate passing runtime across a "DLL boundary"
    fn simulate_dll_function(rt: Runtime) -> i32 {
        let handle = rt.spawn(async { 123 });
        rt.block_on(async move {
            handle.await
        })
    }
    
    let rt = Runtime::new().unwrap();
    let result = simulate_dll_function(rt);
    assert_eq!(result, 123);
}

#[test]
fn test_long_running_tasks() {
    let start = std::time::Instant::now();
    let rt = Runtime::new().unwrap();
    let rt_clone = rt.clone();
    
    rt.block_on(async move {
        let mut handles = Vec::new();
        
        for i in 0..5 {
            let handle = rt_clone.spawn(async move {
                // Simulate some async work
                for _ in 0..1000 {
                    futures::future::ready(()).await;
                }
                i
            });
            handles.push(handle);
        }
        
        let mut sum = 0;
        for handle in handles {
            sum += handle.await;
        }
        
        assert_eq!(sum, 0 + 1 + 2 + 3 + 4);
    });
    
    // Should complete reasonably quickly
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn test_empty_futures() {
    use std::time::{Duration, Instant};
    
    let start = Instant::now();
    let result = std::thread::spawn(|| {
        let rt = Runtime::new().unwrap();
        let rt_clone = rt.clone();
        rt.block_on(async move {
            let handle = rt_clone.spawn(async {});
            handle.await;
            42
        })
    }).join();
    
    // Test should complete within 5 seconds
    assert!(start.elapsed() < Duration::from_secs(5), "Test took too long, likely hung");
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_immediate_ready_futures() {
    let rt = Runtime::new().unwrap();
    let result = rt.block_on(futures::future::ready(123));
    assert_eq!(result, 123);
}

#[test]
fn test_stress_many_tasks() {
    let start = std::time::Instant::now();
    let rt = Runtime::new().unwrap();
    
    rt.clone().block_on(async move {
        let mut handles = Vec::new();
        
        // Spawn many small tasks
        for i in 0..1000 {
            let handle = rt.spawn(async move { i % 10 });
            handles.push(handle);
        }
        
        let mut sum = 0;
        for handle in handles {
            sum += handle.await;
        }
        
        // Sum should be 0+1+2+...+9 repeated 100 times = 45 * 100 = 4500
        assert_eq!(sum, 4500);
    });
    
    // Should complete in reasonable time even with many tasks
    assert!(start.elapsed() < Duration::from_secs(10));
}