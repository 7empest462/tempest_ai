// ==========================================
// 📊 SKG SYSTEM TELEMETRY TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use sysinfo::{Components, System};
use std::sync::atomic::Ordering;

#[cfg(target_os = "macos")]
use tempest_monitor::macos_helper::get_macos_gpu_info;

// ── system_telemetry ───────────────────────────────────────────────────────────

#[skg_tool(
    name = "system_telemetry",
    description = "Performs a COMPLETE SYSTEM DIAGNOSTIC SCAN (Hardware, GPU, Services, Network, SSD). This is your primary tool for all 'checks' and 'scans'. Use extensive=true for deep sweeps."
)]
pub async fn system_telemetry(
    summary_only: Option<bool>,
    extensive: Option<bool>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx.deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    let summary_only_val = summary_only.unwrap_or(false);
    let extensive_val = extensive.unwrap_or(false);

    let mut sys = System::new_with_specifics(sysinfo::RefreshKind::everything());
    sys.refresh_all();

    let components = Components::new_with_refreshed_list();

    let load = System::load_average();
    let total_mem = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let used_mem = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

    #[allow(unused_mut)]
    let mut gpu_temp_str = "Unknown".to_string();
    #[cfg(target_os = "macos")]
    {
        let priority_keys = [
            "TG0p", "TG0D", "TG0P", "TG1p", "TG1D", "Ts0P", "Tp0P", "Tp09", "TA0p",
        ];
        for key in priority_keys {
            if let Some(c) = components.iter().find(|c| c.label() == key)
                && let Some(t) = c.temperature()
                && t > 0.0
            {
                gpu_temp_str = format!("{:.1} °C", t);
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
        load.one,
        load.five,
        load.fifteen,
        used_mem,
        total_mem,
        (used_mem / total_mem * 100.0),
        gpu_temp_str
    );

    if !tool_ctx.is_root.load(Ordering::SeqCst) {
        report.push_str(
            "⚠️  Note: Restricted privileges. Run as sudo for full GPU/SMC matrix.\n",
        );
    }

    if !summary_only_val {
        report.push_str("\n- Full Sensor List:\n");
        let total_sensors = components.len();

        let mut sensor_list = components
            .iter()
            .filter(|c| c.temperature().unwrap_or(0.0) > 0.0)
            .collect::<Vec<_>>();

        sensor_list.sort_by(|a, b| {
            b.temperature()
                .unwrap_or(0.0)
                .partial_cmp(&a.temperature().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let sensor_limit = 15;
        let show_count = std::cmp::min(sensor_list.len(), sensor_limit);

        if show_count > 0 {
            for sensor in &sensor_list[..show_count] {
                #[allow(unused_mut)]
                let mut label = sensor.label().to_string();
                let raw_label = label.clone();

                #[cfg(target_os = "macos")]
                if label.len() == 4 {
                    label = format!(
                        "{} ({})",
                        tempest_monitor::macos_helper::decode_smc_label(&label),
                        label
                    );
                }

                if extensive_val {
                    report.push_str(&format!(
                        "  - {}: {:.1} °C [Raw: {}]\n",
                        label,
                        sensor.temperature().unwrap_or(0.0),
                        raw_label
                    ));
                } else {
                    report.push_str(&format!(
                        "  - {}: {:.1} °C\n",
                        label,
                        sensor.temperature().unwrap_or(0.0)
                    ));
                }
            }
            if total_sensors > sensor_limit {
                report.push_str(
                    "  ... [TRUNCATED] Critical thermal sensors prioritized above.\n",
                );
            }
        } else {
            report.push_str("  - [NONE DETECTED]\n");
        }

        #[cfg(target_os = "macos")]
        {
            let tel = get_macos_gpu_info(false);
            report.push_str(&format!(
            "- GPU (Native): {}\n  - Usage: {:.1}%\n  - GPU Power: {} mW\n  - Package Power: {} mW\n",
            tel.model,
            tel.usage_pct,
            tel.power_mw.map(|v: f64| v.to_string()).unwrap_or_else(|| "-".to_string()),
            tel.package_power_mw.map(|v: f64| v.to_string()).unwrap_or_else(|| "-".to_string())
        ));
        }

        #[cfg(target_os = "linux")]
        {
            let tel = tempest_monitor::linux_helper::collect_gpu_telemetry();
            report.push_str(&format!(
                "- GPU (Unified): {} [Driver: {}]\n",
                tel.model, tel.driver
            ));
            report.push_str(&format!("  - Load: {:.1}%\n", tel.usage_pct));

            if let Some(t) = tel.temp_c {
                report.push_str(&format!("  - Temp: {} °C\n", t));
            }
            if let Some(clk) = tel.clock_mhz {
                report.push_str(&format!("  - Clock: {} MHz\n", clk));
            }
            if let (Some(used), Some(total)) = (tel.vram_used, tel.vram_total) {
                let u_gb = used as f64 / 1024.0 / 1024.0 / 1024.0;
                let t_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
                report.push_str(&format!(
                    "  - VRAM: {:.2} GB / {:.2} GB ({:.1}%)\n",
                    u_gb,
                    t_gb,
                    (u_gb / t_gb * 100.0)
                ));
            }

            if !tel.nvidia_info.is_empty() {
                report.push_str("  - NVIDIA Cluster Details:\n");
                for gpu in tel.nvidia_info {
                    report.push_str(&format!(
                        "    - {}: Temp {} °C, Memory {:.1}%, Power {} mW\n",
                        gpu.name, gpu.temperature, gpu.memory_used_pct, gpu.power_usage_mw
                    ));
                }
            }
        }
    }

    if extensive_val {
        use sysinfo::{Disks, Networks};
        report.push_str("\n--- [ EXTENSIVE TOPOLOGY ] ---\n");

        let services = tempest_monitor::system_helper::get_services();
        let running = services.iter().filter(|s| s.pid.is_some()).count();

        let errors = services
            .iter()
            .filter(|s| {
                if cfg!(target_os = "macos") {
                    s.status != 0
                        && s.status != 1
                        && s.status != 78
                        && s.status != -9
                        && s.status != -15
                } else {
                    s.status != 0
                }
            })
            .count();

        report.push_str(&format!(
            "\n💼 SERVICES: {} Total | {} Running | {} Potential Errors\n",
            services.len(),
            running,
            errors
        ));

        let sockets = tempest_monitor::system_helper::get_sockets(&sys);
        let listening = sockets.iter().filter(|s| s.state == "LISTEN").count();
        let established = sockets.iter().filter(|s| s.state == "ESTABLISHED").count();
        report.push_str(&format!(
            "🕸️  SOCKETS: {} Listening | {} Established\n",
            listening, established
        ));

        report.push_str("\n💾 DISKS\n");
        let disks = Disks::new_with_refreshed_list();
        for disk in &disks {
            report.push_str(&format!(
                "  - {:?} [{:?}] | Mounted at: {:?} | {} / {} GB\n",
                disk.name(),
                disk.kind(),
                disk.mount_point(),
                (disk.total_space() - disk.available_space()) / 1_000_000_000,
                disk.total_space() / 1_000_000_000
            ));
        }

        report.push_str("\n📡 NETWORKS\n");
        let networks = Networks::new_with_refreshed_list();
        for (name, data) in &networks {
            report.push_str(&format!(
                "  - {}: MAC {} | Tx: {}B, Rx: {}B\n",
                name,
                data.mac_address(),
                data.total_transmitted(),
                data.total_received()
            ));
        }
    }

    let proc_count = if extensive_val { 30 } else { 5 };
    report.push_str(&format!(
        "\n💡 Top {} Processes (Memory Hogs):\n",
        proc_count
    ));

    let top_mem = tempest_monitor::process_helper::get_top_memory_processes(proc_count);
    for p in top_mem {
        let mb = p.memory_bytes / 1024 / 1024;
        report.push_str(&format!(
            "  - {} (PID: {}): {} MB | {:.1}% CPU\n",
            p.name, p.pid, mb, p.cpu_usage
        ));
    }

    Ok(serde_json::Value::String(report))
}
