mod sched;

use sched::bust_algorithm_for_real::Tasque;

async fn mewo(hehe: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("meow start");
    if let Err(err) = async_std::fs::remove_dir(hehe).await {
        eprintln!("got error {err}, but who cares");
    }
    println!("mewo remove dir");
    async_std::fs::create_dir(hehe).await?;
    println!("meow");

    Ok(())
}

async fn mewo_wrap(hehe: &str) {
    mewo(hehe).await.unwrap();
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

fn main() {
    println!("hewwo");
    let mut exec = sched::bust_algorithm_for_real::Executioner::new();
    exec.spawn(Tasque::new(mewo_wrap("hehe")));
    exec.spawn(Tasque::new(whats_under_nikos_hat()));
    exec.spawn(Tasque::new(nya()));

    exec.run();
    println!("no more tasks left in queue.");
}

