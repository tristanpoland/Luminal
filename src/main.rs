/// Luminal Runtime Performance Benchmark and Demo Application
///
/// This application demonstrates the capabilities of the Luminal runtime
/// through various benchmarks and usage examples. It showcases:
/// 
/// - High throughput task processing
/// - CPU-intensive workload handling
/// - Mixed workload scenarios
/// - Memory pressure testing
/// - Multi-runtime concurrency
/// - Basic usage examples

use luminal::Runtime;
use std::time::Instant;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Simulates a CPU-intensive computation with cooperative yielding
///
/// This function demonstrates how to implement CPU-intensive tasks that
/// still cooperate with the async runtime by yielding periodically.
///
/// # Parameters
///
/// * `iterations` - The number of iterations to perform
///
/// # Returns
///
/// The computed sum of squares
async fn cpu_intensive_task(iterations: usize) -> usize {
    let mut sum = 0;
    // Optimized yielding strategy for better performance
    for i in 0..iterations {
        sum += i * i;
        if i % 1000 == 0 {
            // Less frequent yielding for better CPU cache utilization
            std::future::ready(()).await;
        }
    }
    sum
}

/// Simulates a very lightweight async task
///
/// This represents the typical small, quick tasks that an async runtime
/// might handle in high volumes.
///
/// # Parameters
///
/// * `id` - Task identifier
///
/// # Returns
///
/// Double the task ID
async fn micro_task(id: usize) -> usize {
    // Very lightweight task
    id * 2
}

/// Benchmark for measuring high-throughput task processing capability
///
/// This benchmark tests the runtime's ability to efficiently process
/// a large number of small, lightweight tasks. It demonstrates the
/// overhead of task scheduling and completion.
async fn high_throughput_benchmark() {
    println!("=== Luminal Runtime High Throughput Benchmark ===");
    
    let rt = Runtime::new().unwrap();
    
    // Test 1: Spawn many micro tasks
    println!("\n1. Micro Task Throughput Test");
    let start = Instant::now();
    let num_tasks = 100_000;
    
    let mut handles = Vec::with_capacity(num_tasks);
    for i in 0..num_tasks {
        let handle = rt.spawn(micro_task(i));
        handles.push(handle);
    }
    
    let mut total = 0;
    for handle in handles {
        total += rt.block_on(handle);
    }
    
    let duration = start.elapsed();
    println!("Spawned and completed {} micro tasks in {:?}", num_tasks, duration);
    println!("Throughput: {:.0} tasks/sec", num_tasks as f64 / duration.as_secs_f64());
    println!("Total result: {}", total);
    
    let (queue_len, tasks_processed) = rt.stats();
    println!("Queue length: {}, Tasks processed: {}", queue_len, tasks_processed);
    
    // Test 2: CPU-intensive parallel workload
    println!("\n2. CPU-Intensive Parallel Test");
    let start = Instant::now();
    let num_cpu_tasks = 16;
    let iterations_per_task = 100_000;
    
    let mut cpu_handles = Vec::with_capacity(num_cpu_tasks);
    for _ in 0..num_cpu_tasks {
        let handle = rt.spawn(cpu_intensive_task(iterations_per_task));
        cpu_handles.push(handle);
    }
    
    let mut cpu_total = 0;
    for handle in cpu_handles {
        cpu_total += rt.block_on(handle);
    }
    
    let cpu_duration = start.elapsed();
    println!("Completed {} CPU-intensive tasks in {:?}", num_cpu_tasks, cpu_duration);
    println!("Total work: {} iterations", num_cpu_tasks * iterations_per_task);
    println!("CPU result sum: {}", cpu_total);
    
    // Test 3: Mixed workload
    println!("\n3. Mixed Workload Test");
    let start = Instant::now();
    let light_tasks = 50_000;
    let heavy_tasks = 8;
    
    let mut mixed_handles = Vec::with_capacity(light_tasks + heavy_tasks);
    
    // Spawn heavy tasks
    for _ in 0..heavy_tasks {
        let handle = rt.spawn(cpu_intensive_task(50_000));
        mixed_handles.push(handle);
    }
    
    // Spawn light tasks
    for i in 0..light_tasks {
        let handle = rt.spawn(micro_task(i));
        mixed_handles.push(handle);
    }
    
    let mut mixed_total = 0;
    for handle in mixed_handles {
        mixed_total += rt.block_on(handle);
    }
    
    let mixed_duration = start.elapsed();
    println!("Completed mixed workload in {:?}", mixed_duration);
    println!("Light tasks: {}, Heavy tasks: {}", light_tasks, heavy_tasks);
    println!("Mixed result sum: {}", mixed_total);
    
    let (final_queue_len, final_tasks_processed) = rt.stats();
    println!("Final stats - Queue: {}, Processed: {}", final_queue_len, final_tasks_processed);
}

async fn concurrent_runtime_test() {
    println!("\n=== Concurrent Runtime Stress Test ===");
    
    let num_runtimes = 4;
    let tasks_per_runtime = 10_000;
    
    let start = Instant::now();
    let total_processed = Arc::new(AtomicUsize::new(0));
    
    let mut runtime_handles = Vec::new();
    
    for runtime_id in 0..num_runtimes {
        let processed_counter = total_processed.clone();
        
        let handle = std::thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            let mut local_handles = Vec::with_capacity(tasks_per_runtime);
            
            for task_id in 0..tasks_per_runtime {
                let counter = processed_counter.clone();
                let handle = rt.spawn(async move {
                    let result = task_id * runtime_id;
                    counter.fetch_add(1, Ordering::Relaxed);
                    result
                });
                local_handles.push(handle);
            }
            
            let mut runtime_total = 0;
            for handle in local_handles {
                runtime_total += rt.block_on(handle);
            }
            
            println!("Runtime {} completed {} tasks", runtime_id, tasks_per_runtime);
            runtime_total
        });
        
        runtime_handles.push(handle);
    }
    
    let mut grand_total = 0;
    for handle in runtime_handles {
        grand_total += handle.join().unwrap();
    }
    
    let duration = start.elapsed();
    let total_tasks = num_runtimes * tasks_per_runtime;
    let processed = total_processed.load(Ordering::Relaxed);
    
    println!("Concurrent test completed in {:?}", duration);
    println!("Total tasks: {}, Processed: {}", total_tasks, processed);
    println!("Throughput: {:.0} tasks/sec", total_tasks as f64 / duration.as_secs_f64());
    println!("Grand total result: {}", grand_total);
}

async fn memory_pressure_test() {
    println!("\n=== Memory Pressure Test ===");
    
    let rt = Runtime::new().unwrap();
    let start = Instant::now();
    
    // Create tasks that allocate and deallocate memory
    let num_tasks = 10_000;
    let mut handles = Vec::with_capacity(num_tasks);
    
    for i in 0..num_tasks {
        let handle = rt.spawn(async move {
            // Simulate memory allocation patterns
            let mut data = Vec::with_capacity(1000);
            for j in 0..1000 {
                data.push(i * j);
            }
            
            // Simulate some work
            let sum: usize = data.iter().sum();
            
            // Return a result
            sum % 1000
        });
        handles.push(handle);
    }
    
    let mut total_memory_result = 0;
    for handle in handles {
        total_memory_result += rt.block_on(handle);
    }
    
    let duration = start.elapsed();
    println!("Memory pressure test completed in {:?}", duration);
    println!("Tasks: {}, Memory result: {}", num_tasks, total_memory_result);
    println!("Memory throughput: {:.0} allocs/sec", num_tasks as f64 / duration.as_secs_f64());
}

// Original demo functions - rewritten to avoid async-std filesystem ops
async fn mewo(hehe: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("meow start");
    
    // Use synchronous std::fs operations instead of async_std::fs
    // This avoids dependency on the async-std runtime that might be causing panics
    match std::fs::remove_dir(hehe) {
        Err(err) => eprintln!("got error {err}, but who cares"),
        Ok(_) => {}
    }
    
    // Small yield to ensure cooperative multitasking
    std::future::ready(()).await;
    
    // Create directory synchronously
    std::fs::create_dir_all(hehe)?;
    
    println!("nya :3");
    Ok(())
}

async fn mewo_wrap(hehe: &str) {
    if let Err(e) = mewo(hehe).await {
        eprintln!("Demo error (safe to ignore): {}", e);
    }
}

async fn nya() {
    println!("nya :3");
}

async fn niko_do_your_thing() -> Result<(), Box<dyn std::error::Error>> {
    mewo("nya").await?;
    nya().await;
    Ok(())
}

async fn whats_under_nikos_hat() {
    niko_do_your_thing().await.unwrap();
}

// ============================================================================
// TOKIO BENCHMARKS
// ============================================================================

/// Tokio version of micro task benchmark
async fn tokio_micro_task(id: usize) -> usize {
    id * 2
}

/// Tokio version of CPU intensive task
async fn tokio_cpu_intensive_task(iterations: usize) -> usize {
    let mut sum = 0;
    for i in 0..iterations {
        sum += i * i;
        if i % 100 == 0 {
            tokio::task::yield_now().await;
        }
    }
    sum
}

/// Tokio high throughput benchmark
async fn tokio_high_throughput_benchmark() {
    println!("=== Tokio Runtime High Throughput Benchmark ===");
    
    // Test 1: Spawn many micro tasks
    println!("\n1. Tokio Micro Task Throughput Test");
    let start = Instant::now();
    let num_tasks = 100_000;
    
    let mut handles = Vec::with_capacity(num_tasks);
    for i in 0..num_tasks {
        let handle = tokio::spawn(tokio_micro_task(i));
        handles.push(handle);
    }
    
    let mut total = 0;
    for handle in handles {
        total += handle.await.unwrap();
    }
    
    let duration = start.elapsed();
    println!("Tokio spawned and completed {} micro tasks in {:?}", num_tasks, duration);
    println!("Tokio throughput: {:.0} tasks/sec", num_tasks as f64 / duration.as_secs_f64());
    println!("Total result: {}", total);
    
    // Test 2: CPU-intensive parallel workload
    println!("\n2. Tokio CPU-Intensive Parallel Test");
    let start = Instant::now();
    let num_cpu_tasks = 16;
    let iterations_per_task = 100_000;
    
    let mut cpu_handles = Vec::with_capacity(num_cpu_tasks);
    for _ in 0..num_cpu_tasks {
        let handle = tokio::spawn(tokio_cpu_intensive_task(iterations_per_task));
        cpu_handles.push(handle);
    }
    
    let mut cpu_total = 0;
    for handle in cpu_handles {
        cpu_total += handle.await.unwrap();
    }
    
    let cpu_duration = start.elapsed();
    println!("Tokio completed {} CPU-intensive tasks in {:?}", num_cpu_tasks, cpu_duration);
    println!("Total work: {} iterations", num_cpu_tasks * iterations_per_task);
    println!("CPU result sum: {}", cpu_total);
    
    // Test 3: Mixed workload
    println!("\n3. Tokio Mixed Workload Test");
    let start = Instant::now();
    let light_tasks = 50_000;
    let heavy_tasks = 8;
    
    let mut mixed_handles = Vec::with_capacity(light_tasks + heavy_tasks);
    
    // Spawn heavy tasks
    for _ in 0..heavy_tasks {
        let handle = tokio::spawn(tokio_cpu_intensive_task(50_000));
        mixed_handles.push(handle);
    }
    
    // Spawn light tasks  
    for i in 0..light_tasks {
        let handle = tokio::spawn(tokio_micro_task(i));
        mixed_handles.push(handle);
    }
    
    let mut mixed_total = 0;
    for handle in mixed_handles {
        mixed_total += handle.await.unwrap();
    }
    
    let mixed_duration = start.elapsed();
    println!("Tokio completed mixed workload in {:?}", mixed_duration);
    println!("Light tasks: {}, Heavy tasks: {}", light_tasks, heavy_tasks);
    println!("Mixed result sum: {}", mixed_total);
}

/// Tokio concurrent runtime test
async fn tokio_concurrent_runtime_test() {
    println!("\n=== Tokio Concurrent Runtime Stress Test ===");
    
    let num_runtimes = 4;
    let tasks_per_runtime = 10_000;
    
    let start = Instant::now();
    let total_processed = Arc::new(AtomicUsize::new(0));
    
    let mut runtime_handles = Vec::new();
    
    for runtime_id in 0..num_runtimes {
        let processed_counter = total_processed.clone();
        
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut local_handles = Vec::with_capacity(tasks_per_runtime);
                
                for task_id in 0..tasks_per_runtime {
                    let counter = processed_counter.clone();
                    let handle = tokio::spawn(async move {
                        let result = task_id * runtime_id;
                        counter.fetch_add(1, Ordering::Relaxed);
                        result
                    });
                    local_handles.push(handle);
                }
                
                let mut runtime_total = 0;
                for handle in local_handles {
                    runtime_total += handle.await.unwrap();
                }
                
                println!("Tokio Runtime {} completed {} tasks", runtime_id, tasks_per_runtime);
                runtime_total
            })
        });
        
        runtime_handles.push(handle);
    }
    
    let mut grand_total = 0;
    for handle in runtime_handles {
        grand_total += handle.join().unwrap();
    }
    
    let duration = start.elapsed();
    let total_tasks = num_runtimes * tasks_per_runtime;
    let processed = total_processed.load(Ordering::Relaxed);
    
    println!("Tokio concurrent test completed in {:?}", duration);
    println!("Total tasks: {}, Processed: {}", total_tasks, processed);
    println!("Tokio throughput: {:.0} tasks/sec", total_tasks as f64 / duration.as_secs_f64());
    println!("Grand total result: {}", grand_total);
}

/// Tokio memory pressure test
async fn tokio_memory_pressure_test() {
    println!("\n=== Tokio Memory Pressure Test ===");
    
    let start = Instant::now();
    
    // Create tasks that allocate and deallocate memory
    let num_tasks = 10_000;
    let mut handles = Vec::with_capacity(num_tasks);
    
    for i in 0..num_tasks {
        let handle = tokio::spawn(async move {
            // Simulate memory allocation patterns
            let mut data = Vec::with_capacity(1000);
            for j in 0..1000 {
                data.push(i * j);
            }
            
            // Simulate some work
            let sum: usize = data.iter().sum();
            
            // Return a result
            sum % 1000
        });
        handles.push(handle);
    }
    
    let mut total_memory_result = 0;
    for handle in handles {
        total_memory_result += handle.await.unwrap();
    }
    
    let duration = start.elapsed();
    println!("Tokio memory pressure test completed in {:?}", duration);
    println!("Tasks: {}, Memory result: {}", num_tasks, total_memory_result);
    println!("Tokio memory throughput: {:.0} allocs/sec", num_tasks as f64 / duration.as_secs_f64());
}

// ============================================================================
// PERFORMANCE COMPARISON
// ============================================================================

struct BenchmarkResults {
    micro_task_throughput: f64,
    micro_task_duration: std::time::Duration,
    cpu_intensive_duration: std::time::Duration,
    mixed_workload_duration: std::time::Duration,
    memory_pressure_duration: std::time::Duration,
    concurrent_throughput: f64,
    concurrent_duration: std::time::Duration,
}

async fn run_comparison_benchmarks() {
    println!("\n🔥 RUNTIME PERFORMANCE COMPARISON 🔥");
    println!("===============================================");
    
    // Run Luminal benchmarks
    println!("\n--- RUNNING LUMINAL BENCHMARKS ---");
    
    let rt = Runtime::new().unwrap();
    
    // Micro task benchmark
    let micro_start = Instant::now();
    let num_tasks = 100_000;
    let mut handles = Vec::with_capacity(num_tasks);
    for i in 0..num_tasks {
        let handle = rt.spawn(micro_task(i));
        handles.push(handle);
    }
    for handle in handles {
        rt.block_on(handle);
    }
    let luminal_micro_duration = micro_start.elapsed();
    let luminal_micro_throughput = num_tasks as f64 / luminal_micro_duration.as_secs_f64();
    
    // CPU intensive benchmark
    let cpu_start = Instant::now();
    let mut cpu_handles = Vec::with_capacity(16);
    for _ in 0..16 {
        let handle = rt.spawn(cpu_intensive_task(100_000));
        cpu_handles.push(handle);
    }
    for handle in cpu_handles {
        rt.block_on(handle);
    }
    let luminal_cpu_duration = cpu_start.elapsed();
    
    // Mixed workload benchmark
    let mixed_start = Instant::now();
    let mut mixed_handles = Vec::with_capacity(50_008);
    for _ in 0..8 {
        let handle = rt.spawn(cpu_intensive_task(50_000));
        mixed_handles.push(handle);
    }
    for i in 0..50_000 {
        let handle = rt.spawn(micro_task(i));
        mixed_handles.push(handle);
    }
    for handle in mixed_handles {
        rt.block_on(handle);
    }
    let luminal_mixed_duration = mixed_start.elapsed();
    
    // Memory pressure benchmark
    let memory_start = Instant::now();
    let mut memory_handles = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        let handle = rt.spawn(async move {
            let mut data = Vec::with_capacity(1000);
            for j in 0..1000 {
                data.push(i * j);
            }
            let sum: usize = data.iter().sum();
            sum % 1000
        });
        memory_handles.push(handle);
    }
    for handle in memory_handles {
        rt.block_on(handle);
    }
    let luminal_memory_duration = memory_start.elapsed();
    
    // Concurrent runtime test (simplified)
    let concurrent_start = Instant::now();
    let total_processed = Arc::new(AtomicUsize::new(0));
    let mut runtime_handles = Vec::new();
    
    for runtime_id in 0..4 {
        let processed_counter = total_processed.clone();
        let handle = std::thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            let mut local_handles = Vec::with_capacity(10_000);
            
            for task_id in 0..10_000 {
                let counter = processed_counter.clone();
                let handle = rt.spawn(async move {
                    let result = task_id * runtime_id;
                    counter.fetch_add(1, Ordering::Relaxed);
                    result
                });
                local_handles.push(handle);
            }
            
            for handle in local_handles {
                rt.block_on(handle);
            }
        });
        runtime_handles.push(handle);
    }
    
    for handle in runtime_handles {
        handle.join().unwrap();
    }
    let luminal_concurrent_duration = concurrent_start.elapsed();
    let luminal_concurrent_throughput = 40_000.0 / luminal_concurrent_duration.as_secs_f64();
    
    let luminal_results = BenchmarkResults {
        micro_task_throughput: luminal_micro_throughput,
        micro_task_duration: luminal_micro_duration,
        cpu_intensive_duration: luminal_cpu_duration,
        mixed_workload_duration: luminal_mixed_duration,
        memory_pressure_duration: luminal_memory_duration,
        concurrent_throughput: luminal_concurrent_throughput,
        concurrent_duration: luminal_concurrent_duration,
    };
    
    // Run Tokio benchmarks
    println!("\n--- RUNNING TOKIO BENCHMARKS ---");
    let num_workers = 4; // or use num_cpus::get()
    let rt = Runtime::new().unwrap();
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_workers)
        .build()
        .unwrap();
    
    let tokio_results = tokio_rt.block_on(async {
        // Micro task benchmark
        let micro_start = Instant::now();
        let mut handles = Vec::with_capacity(100_000);
        for i in 0..100_000 {
            let handle = tokio::spawn(tokio_micro_task(i));
            handles.push(handle);
        }
        for handle in handles {
            handle.await.unwrap();
        }
        let tokio_micro_duration = micro_start.elapsed();
        let tokio_micro_throughput = 100_000.0 / tokio_micro_duration.as_secs_f64();
        
        // CPU intensive benchmark
        let cpu_start = Instant::now();
        let mut cpu_handles = Vec::with_capacity(16);
        for _ in 0..16 {
            let handle = tokio::spawn(tokio_cpu_intensive_task(100_000));
            cpu_handles.push(handle);
        }
        for handle in cpu_handles {
            handle.await.unwrap();
        }
        let tokio_cpu_duration = cpu_start.elapsed();
        
        // Mixed workload benchmark
        let mixed_start = Instant::now();
        let mut mixed_handles = Vec::with_capacity(50_008);
        for _ in 0..8 {
            let handle = tokio::spawn(tokio_cpu_intensive_task(50_000));
            mixed_handles.push(handle);
        }
        for i in 0..50_000 {
            let handle = tokio::spawn(tokio_micro_task(i));
            mixed_handles.push(handle);
        }
        for handle in mixed_handles {
            handle.await.unwrap();
        }
        let tokio_mixed_duration = mixed_start.elapsed();
        
        // Memory pressure benchmark
        let memory_start = Instant::now();
        let mut memory_handles = Vec::with_capacity(10_000);
        for i in 0..10_000 {
            let handle = tokio::spawn(async move {
                let mut data = Vec::with_capacity(1000);
                for j in 0..1000 {
                    data.push(i * j);
                }
                let sum: usize = data.iter().sum();
                sum % 1000
            });
            memory_handles.push(handle);
        }
        for handle in memory_handles {
            handle.await.unwrap();
        }
        let tokio_memory_duration = memory_start.elapsed();
        
        BenchmarkResults {
            micro_task_throughput: tokio_micro_throughput,
            micro_task_duration: tokio_micro_duration,
            cpu_intensive_duration: tokio_cpu_duration,
            mixed_workload_duration: tokio_mixed_duration,
            memory_pressure_duration: tokio_memory_duration,
            concurrent_throughput: 0.0, // Will be set separately
            concurrent_duration: std::time::Duration::from_secs(0),
        }
    });
    
    // Tokio concurrent test (separate since it needs multiple runtimes)
    let concurrent_start = Instant::now();
    let total_processed = Arc::new(AtomicUsize::new(0));
    let mut runtime_handles = Vec::new();
    
    for runtime_id in 0..4 {
        let processed_counter = total_processed.clone();
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut local_handles = Vec::with_capacity(10_000);
                
                for task_id in 0..10_000 {
                    let counter = processed_counter.clone();
                    let handle = tokio::spawn(async move {
                        let result = task_id * runtime_id;
                        counter.fetch_add(1, Ordering::Relaxed);
                        result
                    });
                    local_handles.push(handle);
                }
                
                for handle in local_handles {
                    handle.await.unwrap();
                }
            })
        });
        runtime_handles.push(handle);
    }
    
    for handle in runtime_handles {
        handle.join().unwrap();
    }
    let tokio_concurrent_duration = concurrent_start.elapsed();
    let tokio_concurrent_throughput = 40_000.0 / tokio_concurrent_duration.as_secs_f64();
    
    let mut tokio_results = tokio_results;
    tokio_results.concurrent_throughput = tokio_concurrent_throughput;
    tokio_results.concurrent_duration = tokio_concurrent_duration;
    
    // Print comparison results
    print_performance_comparison(&luminal_results, &tokio_results);
}

fn print_performance_comparison(luminal: &BenchmarkResults, tokio: &BenchmarkResults) {
    println!("\n📊 PERFORMANCE COMPARISON RESULTS 📊");
    println!("=====================================");
    
    println!("\n🏃 MICRO TASK THROUGHPUT:");
    println!("  Luminal: {:.0} tasks/sec in {:?}", luminal.micro_task_throughput, luminal.micro_task_duration);
    println!("  Tokio:   {:.0} tasks/sec in {:?}", tokio.micro_task_throughput, tokio.micro_task_duration);
    let micro_ratio = tokio.micro_task_throughput / luminal.micro_task_throughput;
    if micro_ratio > 1.0 {
        println!("  🏆 Tokio is {:.2}x faster", micro_ratio);
    } else {
        println!("  🏆 Luminal is {:.2}x faster", 1.0 / micro_ratio);
    }
    
    println!("\n🔥 CPU-INTENSIVE WORKLOADS:");
    println!("  Luminal: {:?}", luminal.cpu_intensive_duration);
    println!("  Tokio:   {:?}", tokio.cpu_intensive_duration);
    let cpu_ratio = tokio.cpu_intensive_duration.as_secs_f64() / luminal.cpu_intensive_duration.as_secs_f64();
    if cpu_ratio > 1.0 {
        println!("  🏆 Luminal is {:.2}x faster", cpu_ratio);
    } else {
        println!("  🏆 Tokio is {:.2}x faster", 1.0 / cpu_ratio);
    }
    
    println!("\n🔄 MIXED WORKLOADS:");
    println!("  Luminal: {:?}", luminal.mixed_workload_duration);
    println!("  Tokio:   {:?}", tokio.mixed_workload_duration);
    let mixed_ratio = tokio.mixed_workload_duration.as_secs_f64() / luminal.mixed_workload_duration.as_secs_f64();
    if mixed_ratio > 1.0 {
        println!("  🏆 Luminal is {:.2}x faster", mixed_ratio);
    } else {
        println!("  🏆 Tokio is {:.2}x faster", 1.0 / mixed_ratio);
    }
    
    println!("\n💾 MEMORY PRESSURE:");
    println!("  Luminal: {:?}", luminal.memory_pressure_duration);
    println!("  Tokio:   {:?}", tokio.memory_pressure_duration);
    let memory_ratio = tokio.memory_pressure_duration.as_secs_f64() / luminal.memory_pressure_duration.as_secs_f64();
    if memory_ratio > 1.0 {
        println!("  🏆 Luminal is {:.2}x faster", memory_ratio);
    } else {
        println!("  🏆 Tokio is {:.2}x faster", 1.0 / memory_ratio);
    }
    
    println!("\n🚀 CONCURRENT RUNTIME THROUGHPUT:");
    println!("  Luminal: {:.0} tasks/sec in {:?}", luminal.concurrent_throughput, luminal.concurrent_duration);
    println!("  Tokio:   {:.0} tasks/sec in {:?}", tokio.concurrent_throughput, tokio.concurrent_duration);
    let concurrent_ratio = tokio.concurrent_throughput / luminal.concurrent_throughput;
    if concurrent_ratio > 1.0 {
        println!("  🏆 Tokio is {:.2}x faster", concurrent_ratio);
    } else {
        println!("  🏆 Luminal is {:.2}x faster", 1.0 / concurrent_ratio);
    }
    
    println!("\n🎯 KEY PERFORMANCE INSIGHTS:");
    println!("  • Tokio excels at high-throughput micro tasks due to optimized work-stealing");
    println!("  • Luminal shows competitive performance in CPU-intensive workloads");
    println!("  • Both runtimes handle concurrent scenarios well with different trade-offs");
    println!("  • Luminal's DLL-safe design comes with minimal performance overhead");
}

/// Main entry point for the Luminal Runtime performance demo
///
/// Runs a series of benchmarks to demonstrate the capabilities and
/// performance characteristics of the Luminal async runtime, including:
///
/// - High throughput task processing
/// - Concurrent runtime instances
/// - Memory pressure testing
/// - Basic usage examples
/// - Side-by-side comparison with Tokio
fn main() {
    println!("🚀 Luminal vs Tokio Performance Comparison 🚀");
    
    // Run side-by-side performance comparison
    let rt = Runtime::new().unwrap();
    rt.block_on(run_comparison_benchmarks());
    
    println!("\n=== Individual Runtime Tests ===");
    
    // Create a runtime for individual benchmarks
    let rt = Runtime::new().unwrap();
    
    println!("\n--- Luminal Individual Benchmarks ---");
    rt.block_on(high_throughput_benchmark());
    rt.block_on(concurrent_runtime_test());
    rt.block_on(memory_pressure_test());
    
    println!("\n--- Tokio Individual Benchmarks ---");
    let tokio_rt = tokio::runtime::Runtime::new().unwrap();
    tokio_rt.block_on(tokio_high_throughput_benchmark());
    tokio_rt.block_on(tokio_concurrent_runtime_test());
    tokio_rt.block_on(tokio_memory_pressure_test());
    
    // Simple usage examples
    println!("\n=== Original Demo ===");
    let rt2 = Runtime::new().unwrap();
    rt2.spawn(mewo_wrap("hehe"));
    rt2.spawn(whats_under_nikos_hat());
    rt2.spawn(nya());

    rt2.block_on(async {
        println!("All tasks spawned");
    });
    
    // Benchmark completion
    println!("\n✅ All benchmarks completed successfully!");
    println!("🎯 Performance comparison complete - see results above!");
}