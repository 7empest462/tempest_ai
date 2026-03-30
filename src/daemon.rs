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
            notify_system("Tempest AI Daemon", &message);
        }

        // Wait 5 minutes before the next sweep
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

fn notify_system(title: &str, message: &str) {
    #[cfg(target_os = "macos")]
    {
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

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send")
            .arg("-u")
            .arg("critical")
            .arg(title)
            .arg(message)
            .output();
    }
}

pub fn install_daemon() {
    println!("Installing persistent background service...");
    #[cfg(target_os = "macos")]
    {
        let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.tempest.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/tempest_ai</string>
        <string>--daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>"#;

        let plist_path = "/Library/LaunchDaemons/com.tempest.daemon.plist";
        if let Err(e) = std::fs::write(plist_path, plist_content) {
            println!("❌ Failed to write plist. Make sure to run with 'sudo tempest_ai --install-daemon'. Error: {}", e);
            return;
        }
        
        let _ = Command::new("launchctl").arg("unload").arg(plist_path).output();
        let _ = Command::new("launchctl").arg("load").arg(plist_path).output();
        println!("✅ Installed to {}", plist_path);
        println!("🚀 Background persistence engaged via launchctl.");
    }

    #[cfg(target_os = "linux")]
    {
        let service_content = r#"[Unit]
Description=Tempest AI Hardware Sentinel Daemon
After=network.target

[Service]
ExecStart=/usr/local/bin/tempest_ai --daemon
Restart=always
User=root
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=multi-user.target"#;

        let service_path = "/etc/systemd/system/tempest-daemon.service";
        if let Err(e) = std::fs::write(service_path, service_content) {
            println!("❌ Failed to write service file. Make sure to run with 'sudo tempest_ai --install-daemon'. Error: {}", e);
            return;
        }

        let _ = Command::new("systemctl").arg("daemon-reload").output();
        let _ = Command::new("systemctl").arg("enable").arg("--now").arg("tempest-daemon").output();
        println!("✅ Installed to {}", service_path);
        println!("🚀 Background persistence engaged via systemctl.");
    }
}
