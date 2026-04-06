use serde_json::Value;
use anyhow::Result;
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
}

pub struct SystemTelemetryTool;

#[async_trait]
impl AgentTool for SystemTelemetryTool {
    fn name(&self) -> &'static str { "get_system_telemetry" }
    fn description(&self) -> &'static str { 
        "Returns high-fidelity system telemetry FROM TEMPEST MONITOR. Use this for ALL hardware diagnostics on macOS, including GPU Temperature."
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

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: SystemTelemetryArgs = serde_json::from_value(args.clone()).unwrap_or(SystemTelemetryArgs { summary_only: Some(false) });
        let summary_only = typed_args.summary_only.unwrap_or(false);

        let mut sys = System::new_with_specifics(sysinfo::RefreshKind::everything());
        sys.refresh_all();

        let components = Components::new();
        
        let load = System::load_average();
        let total_mem = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_mem = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        
        // 🧪 PRE-CALCULATE GPU TEMP (macOS)
        let mut gpu_temp_str = "Unknown".to_string();
        #[cfg(target_os = "macos")]
        {
            for c in &components {
                let label = c.label();
                if label == "TG0D" || label == "TG0P" {
                    gpu_temp_str = format!("{:.1} °C", c.temperature().unwrap_or(0.0));
                    break;
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

        if !summary_only {
            // Termals (Sensors)
            report.push_str("\n- Full Sensor List:\n");
        let sensor_list: &Components = &components;
        for component in sensor_list {
            let mut label = component.label().to_string();
            
            #[cfg(target_os = "macos")]
            if label.len() == 4 {
                label = format!("{} ({})", tempest_monitor::macos_helper::decode_smc_label(&label), label);
            }
            
            report.push_str(&format!("  - {}: {:.1} °C\n", label, component.temperature().unwrap_or(0.0)));
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

        // 🧠 SURGICAL INTELLIGENCE: Top Processes (Always shown in summary)
        report.push_str("\n💡 Top 5 Processes (Memory Hogs):\n");
        let top_mem = tempest_monitor::process_helper::get_top_memory_processes(5);
        for p in top_mem {
            let mb = p.memory_bytes / 1024 / 1024;
            report.push_str(&format!("  - {} (PID: {}): {} MB | {:.1}% CPU\n", p.name, p.pid, mb, p.cpu_usage));
        }

        Ok(report)
    }
}
