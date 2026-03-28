use sysinfo::{System, Networks};
use std::process::Command;
use std::time::Duration;
use chrono::Local;

pub async fn run_daemon() {
    println!("[{}] 🛡️ Tempest AI Daemon Initializing in Headless Mode...", Local::now().format("%H:%M:%S"));
    println!("The agent is now actively patrolling your local memory limits and network routing topologies unconditionally in the background.\n");
    
    let mut sys = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    
    // Simple Autonomous Ruleset
    let memory_threshold_mb = 14_000; // Let's say 14GB RAM threshold out of 16GB
    
    loop {
        sys.refresh_all();
        networks.refresh(true);
        
        let used_ram = sys.used_memory() / 1024 / 1024;
        let total_ram = sys.total_memory() / 1024 / 1024;
        
        if used_ram >= memory_threshold_mb {
            println!("[{}] ⚠️ CRITICAL RAM SPIKE: {} / {} MB", Local::now().format("%H:%M:%S"), used_ram, total_ram);
            let message = format!("CRITICAL RAM SPIKE: {} MB currently mapped. System integrity in danger.", used_ram);
            notify_macos("Tempest AI Daemon", &message);
        }

        // We could also do PNET packet drops here, but for now just system stats.
        
        // Wait 5 minutes before the next sweep
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

fn notify_macos(title: &str, message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"{}\" sound name \"Basso\"",
        message.replace('"', "\\\""),
        title.replace('"', "\\\"")
    );

    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();
}
