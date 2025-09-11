use luminal::Runtime;

#[test]
fn simple_test() {
    let rt = Runtime::new().unwrap();
    let rt_clone = rt.clone();
    let result = rt.block_on(async move {
        let handle = rt_clone.spawn(async { 42 });
        handle.await
    });
    assert_eq!(result, 42);
    println!("SUCCESS: Simple test passed!");
}