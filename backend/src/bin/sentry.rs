use backend::sentry::SentryAudit;
use std::env;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut sentry = SentryAudit::new();

    if args.contains(&"--once".to_string()) {
        let report = sentry.run();
        println!("{}", report);
    } else {
        println!("Starting Sentry Audit Daemon (Rust)...");
        loop {
            let report = sentry.run();
            println!("{}", report);
            thread::sleep(Duration::from_secs(60));
        }
    }
}
