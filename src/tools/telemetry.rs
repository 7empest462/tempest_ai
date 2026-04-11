use serde_json::Value;
use miette::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use sysinfo::{System, Components};

#[cfg(target_os = "macos")]
use tempest_monitor::macos_helper::get_macos_gpu_info;

#[derive(Deserialize, JsonSchema)]
pub struct SystemTelemetryArgs {
    /// If true, only returns a high-level dashboard (CPU, RAM, GPU temp, Top 5) and skips the full sensor list.
    pub summary_only: Option<bool>,
    /// If true, performs a deep 3D topology sweep including disks and network interfaces (replaces system_oracle_3d).
    pub extensive: Option<bool>,
}

pub struct SystemTelemetryTool;

#[async_trait]
impl AgentTool for SystemTelemetryTool {
    fn name(&self) -> &'static str { "system_diagnostic_scan" }
    fn description(&self) -> &'static str { 
        "Performs a COMPLETE SYSTEM DIAGNOSTIC SCAN (Hardware, GPU, Services, Network, SSD). This is your primary tool for all 'checks' and 'scans'. Use extensive=true for deep sweeps."
    }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<SystemTelemetryArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: SystemTelemetryArgs = serde_json::from_value(args.clone()).unwrap_or(SystemTelemetryArgs { 
            summary_only: Some(false),
            extensive: Some(false) 
        });
        let summary_only = typed_args.summary_only.unwrap_or(false);
        let extensive = typed_args.extensive.unwrap_or(false);

        let mut sys = System::new_with_specifics(sysinfo::RefreshKind::everything());
        sys.refresh_all();

        let components = Components::new_with_refreshed_list();
        
        let load = System::load_average();
        let total_mem = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_mem = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        
        // 🧪 PRE-CALCULATE GPU TEMP (macOS only — mut is used inside cfg block)
        #[allow(unused_mut)]
        let mut gpu_temp_str = "Unknown".to_string();
        #[cfg(target_os = "macos")]
        {
            // Apple Silicon GPU sensors often use TG0p, TG1p, TG0D, TG1D, Ts0P, Tp0P, Tp09
            let priority_keys = ["TG0p", "TG0D", "TG0P", "TG1p", "TG1D", "Ts0P", "Tp0P", "Tp09", "TA0p"];
            for key in priority_keys {
                if let Some(c) = components.iter().find(|c| c.label() == key) {
                    if let Some(t) = c.temperature() {
                        if t > 0.0 {
                            gpu_temp_str = format!("{:.1} °C", t);
                            break;
                        }
                    }
                }
            }
        }

        let mut report = format!(
            "--- [ TEMPEST MONITOR DASHBOARD ] ---\n\
             - CPU: {:.1}% avg (Load: {} {} {})\n\
             - RAM: {:.2} GB / {:.2} GB ({:.1}%)\n\
             - GPU Temperature: {}\n",
            sys.global_cpu_usage(),
            load.one, load.five, load.fifteen,
            used_mem, total_mem, (used_mem/total_mem * 100.0),
            gpu_temp_str
        );

        use std::sync::atomic::Ordering;
        if !context.is_root.load(Ordering::SeqCst) {
            report.push_str("⚠️  Note: Restricted privileges. Run as sudo for full GPU/SMC matrix.\n");
        }

        if !summary_only {
            // Termals (Sensors)
            report.push_str("\n- Full Sensor List:\n");
            let total_sensors = components.len();
            
            let mut sensor_list = components.iter()
                .filter(|c| c.temperature().unwrap_or(0.0) > 0.0)
                .collect::<Vec<_>>();
                
            // 🔥 Priority: Sort by temperature descending to catch overheating first
            sensor_list.sort_by(|a, b| b.temperature().unwrap_or(0.0).partial_cmp(&a.temperature().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));

            let sensor_limit = 15;
            let show_count = std::cmp::min(sensor_list.len(), sensor_limit);

            if show_count > 0 {
                for sensor in &sensor_list[..show_count] {
                    #[allow(unused_mut)]
                    let mut label = sensor.label().to_string();
                    let raw_label = label.clone();
                    
                    #[cfg(target_os = "macos")]
                    if label.len() == 4 {
                        label = format!("{} ({})", tempest_monitor::macos_helper::decode_smc_label(&label), label);
                    }
                    
                    if extensive {
                        report.push_str(&format!("  - {}: {:.1} °C [Raw: {}]\n", label, sensor.temperature().unwrap_or(0.0), raw_label));
                    } else {
                        report.push_str(&format!("  - {}: {:.1} °C\n", label, sensor.temperature().unwrap_or(0.0)));
                    }
                }
                if total_sensors > sensor_limit {
                    report.push_str("  ... [TRUNCATED] Critical thermal sensors prioritized above.\n");
                }
            } else {
                 report.push_str("  - [NONE DETECTED]\n");
            }

        // Platform-specific GPU Telemetry (from tempest-monitor crate)
        #[cfg(target_os = "macos")]
        {
            let (usage, gpu_mw, cpu_mw) = get_macos_gpu_info();
            report.push_str(&format!(
                "- GPU (Native):\n  - Usage: {:.1}%\n  - GPU Power: {} mW\n  - CPU Power: {} mW\n",
                usage, 
                gpu_mw.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                cpu_mw.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
            ));
        }

        #[cfg(target_os = "linux")]
        {
            let gpu_info = tempest_monitor::linux_helper::get_nvidia_gpu_info();
            if !gpu_info.is_empty() {
                report.push_str("- NVIDIA GPUs:\n");
                for gpu in gpu_info {
                    report.push_str(&format!(
                        "  - {}: Temp {} °C, Memory {:.1}%, Power {} mW\n",
                        gpu.name, gpu.temperature, gpu.memory_used_pct, gpu.power_usage_mw
                    ));
                }
            }
        }
        }

        if extensive {
            use sysinfo::{Disks, Networks};
            report.push_str("\n--- [ EXTENSIVE TOPOLOGY ] ---\n");
            
            // 🚀 SERVICES (from tempest-monitor)
            let services = tempest_monitor::system_helper::get_services();
            let running = services.iter().filter(|s| s.pid.is_some()).count();
            
            // Refine error check: ignore common Launchd non-zero codes (1, 78) that aren't necessarily failures
            let errors = services.iter().filter(|s| {
                if cfg!(target_os = "macos") {
                    s.status != 0 && s.status != 1 && s.status != 78 && s.status != 0
                } else {
                    s.status != 0
                }
            }).count();
            
            report.push_str(&format!("\n💼 SERVICES: {} Total | {} Running | {} Potential Errors\n", services.len(), running, errors));

            // 🌐 SOCKETS (from tempest-monitor)
            let sockets = tempest_monitor::system_helper::get_sockets(&sys);
            let listening = sockets.iter().filter(|s| s.state == "LISTEN").count();
            let established = sockets.iter().filter(|s| s.state == "ESTABLISHED").count();
            report.push_str(&format!("🕸️  SOCKETS: {} Listening | {} Established\n", listening, established));

            report.push_str("\n💾 DISKS\n");
            let disks = Disks::new_with_refreshed_list();
            for disk in &disks {
                report.push_str(&format!("  - {:?} [{:?}] | Mounted at: {:?} | {} / {} GB\n", 
                    disk.name(), disk.kind(), disk.mount_point(),
                    (disk.total_space() - disk.available_space()) / 1_000_000_000,
                    disk.total_space() / 1_000_000_000
                ));
            }

            report.push_str("\n📡 NETWORKS\n");
            let networks = Networks::new_with_refreshed_list();
            for (name, data) in &networks {
                report.push_str(&format!("  - {}: MAC {} | Tx: {}B, Rx: {}B\n", name, data.mac_address(), data.total_transmitted(), data.total_received()));
            }
        }

        // 🧠 SURGICAL INTELLIGENCE: Top Processes
        let proc_count = if extensive { 30 } else { 5 };
        report.push_str(&format!("\n💡 Top {} Processes (Memory Hogs):\n", proc_count));
        
        let top_mem = tempest_monitor::process_helper::get_top_memory_processes(proc_count);
        for p in top_mem {
            let mb = p.memory_bytes / 1024 / 1024;
            report.push_str(&format!("  - {} (PID: {}): {} MB | {:.1}% CPU\n", p.name, p.pid, mb, p.cpu_usage));
        }

        Ok(report)
    }
}
