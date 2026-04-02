use anyhow::Result;
use serde_json::Value;
use crate::tools::AgentTool;

pub struct LinuxProcessAnalyzerTool;

#[async_trait::async_trait]
impl AgentTool for LinuxProcessAnalyzerTool {
    fn name(&self) -> &'static str { "linux_process_analyzer" }
    fn description(&self) -> &'static str { "Read detailed process memory maps, IO counters, and thread counts directly from the Linux kernel." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pid": { "type": "integer", "description": "The target Process ID to analyze." }
            },
            "required": ["pid"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        #[cfg(target_os = "linux")]
        {
            use procfs::WithCurrentSystemInfo;
            let pid = args.get("pid").and_then(|p| p.as_i64()).ok_or_else(|| anyhow::anyhow!("Missing 'pid'"))? as i32;
            let process = procfs::process::Process::new(pid)?;
            
            let stat = process.stat()?;
            let io = process.io()?;
            let cmdline = process.cmdline()?.join(" ");

            let mut out = format!("Process [{}] - {}\n", pid, cmdline);
            out.push_str(&format!("State: {:?}\n", stat.state));
            out.push_str(&format!("Threads: {}\n", stat.num_threads));
            out.push_str(&format!("RSS Memory: {} bytes\n", stat.rss_bytes().get()));
            out.push_str(&format!("Char Read / Write: {} / {} bytes\n", io.rchar, io.wchar));
            
            Ok(out)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = args;
            Ok("Error: The linux_process_analyzer tool relies on the pristine Linux procfs kernel mapping. You are currently running on macOS (Darwin). Use `system_info` or `run_command` with macOS specific polling instead.".to_string())
        }
    }
}

pub struct GpuDiagnosticsTool;

#[async_trait::async_trait]
impl AgentTool for GpuDiagnosticsTool {
    fn name(&self) -> &'static str { "gpu_diagnostics" }
    fn description(&self) -> &'static str { "Read Nvidia GPU telemetry (temperature, clock speeds, active instances) natively." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "gpu_id": { "type": "integer", "description": "Optional GPU ID (default 0).", "default": 0 }
            }
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        #[cfg(target_os = "linux")]
        {
            let gpu_id = args.get("gpu_id").and_then(|g| g.as_i64()).unwrap_or(0) as u32;
            
            let nvml = match nvml_wrapper::Nvml::init() {
                Ok(n) => n,
                Err(e) => return Ok(format!("Failed to initialize NVML (Are Nvidia drivers accessible?): {}", e)),
            };

            let device = match nvml.device_by_index(gpu_id) {
                Ok(d) => d,
                Err(e) => return Ok(format!("Failed to get GPU {}: {}", gpu_id, e)),
            };

            let name = device.name().unwrap_or_else(|_| "Unknown Nvidia GPU".to_string());
            let temp = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                .map(|t| format!("{}°C", t)).unwrap_or_else(|_| "N/A".to_string());
            
            let util = device.utilization_rates();
            let util_gpu = util.as_ref().map(|u| format!("{}%", u.gpu)).unwrap_or_else(|_| "N/A".to_string());
            let util_mem = util.as_ref().map(|u| format!("{}%", u.memory)).unwrap_or_else(|_| "N/A".to_string());
            
            let mem = device.memory_info().ok();
            let mem_used = mem.as_ref().map(|m| format!("{} MB", m.used / 1024 / 1024)).unwrap_or_else(|| "N/A".to_string());
            let mem_total = mem.as_ref().map(|m| format!("{} MB", m.total / 1024 / 1024)).unwrap_or_else(|| "N/A".to_string());

            let mut out = format!("Device {}: {}\n", gpu_id, name);
            out.push_str(&format!("Temperature: {}\n", temp));
            out.push_str(&format!("Utilization: GPU {} | VRAM {}\n", util_gpu, util_mem));
            out.push_str(&format!("Memory Usage: {} / {}\n", mem_used, mem_total));
            
            Ok(out)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = args;
            Ok("Error: The gpu_diagnostics tool maps exclusively to the Nvidia Hardware Management Library. Your current host is an Apple Silicon Mac without an Nvidia GPU.".to_string())
        }
    }
}

pub struct TelemetryChartTool;

#[async_trait::async_trait]
impl AgentTool for TelemetryChartTool {
    fn name(&self) -> &'static str { "generate_telemetry_chart" }
    fn description(&self) -> &'static str { "Generate a high-quality .png line-chart from arrays of X/Y data points. Useful for graphing CPU hogs, memory over time, or network spikes. CRITICAL: data_points MUST be raw numbers (e.g. 0.707), DO NOT put JavaScript math expressions like Math.sin() in the JSON." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Chart Title" },
                "x_label": { "type": "string", "description": "X-axis Label" },
                "y_label": { "type": "string", "description": "Y-axis Label" },
                "series_name": { "type": "string", "description": "Name of the line series" },
                "data_points": {
                    "type": "array",
                    "description": "An array of precise [X, Y] arrays (e.g. [[1, 5], [2, 10], [3, 20]]). CRITICAL: MUST BE RAW FLOATS. DO NOT use expressions like Math.sin()",
                    "items": {
                        "type": "array",
                        "items": { "type": "number" }
                    }
                }
            },
            "required": ["title", "x_label", "y_label", "series_name", "data_points"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("Telemetry Chart");
        let x_label = args.get("x_label").and_then(|x| x.as_str()).unwrap_or("X");
        let y_label = args.get("y_label").and_then(|y| y.as_str()).unwrap_or("Y");
        let series_name = args.get("series_name").and_then(|s| s.as_str()).unwrap_or("Data");
        
        // Parse data points
        let data_arr = args.get("data_points")
            .and_then(|a| a.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'data_points' array"))?;
            
        let mut points: Vec<(f64, f64)> = Vec::new();
        for p in data_arr {
            if let Some(arr) = p.as_array() {
                if arr.len() == 2 {
                    let x = arr[0].as_f64().unwrap_or(0.0);
                    let y = arr[1].as_f64().unwrap_or(0.0);
                    points.push((x, y));
                }
            }
        }

        if points.is_empty() {
            return Ok("Error: No valid data points provided.".to_string());
        }

        // Find min/max for chart auto-scaling
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for (x, y) in &points {
            if *x < min_x { min_x = *x; }
            if *x > max_x { max_x = *x; }
            if *y < min_y { min_y = *y; }
            if *y > max_y { max_y = *y; }
        }
        
        // Add minimal padding
        let x_padding = (max_x - min_x) * 0.05;
        let y_padding = (max_y - min_y) * 0.05;

        min_x -= x_padding;
        max_x += x_padding;
        min_y -= y_padding;
        max_y += y_padding;
        
        // Ensure bounds aren't invalid if min == max
        if min_x == max_x { max_x += 1.0; min_x -= 1.0; }
        if min_y == max_y { max_y += 1.0; min_y -= 1.0; }

        let path = format!("/tmp/tempest_chart_{}.png", format!("{}", chrono::Local::now().format("%H%M%S")));
        
        // Actually draw using Plotters
        use plotters::prelude::*;
        let root = BitMapBackend::new(&path, (800, 600)).into_drawing_area();
        root.fill(&WHITE).map_err(|e| anyhow::anyhow!("Plotters Error: {}", e))?;

        let mut chart = ChartBuilder::on(&root)
            .caption(title, ("sans-serif", 30).into_font())
            .margin(20)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(min_x..max_x, min_y..max_y)
            .map_err(|e| anyhow::anyhow!("Plotters Error: {}", e))?;

        chart.configure_mesh()
            .x_desc(x_label)
            .y_desc(y_label)
            .draw()
            .map_err(|e| anyhow::anyhow!("Plotters Error: {}", e))?;

        chart.draw_series(LineSeries::new(points, &RED))
            .map_err(|e| anyhow::anyhow!("Plotters Error: {}", e))?
            .label(series_name)
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

        chart.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()
            .map_err(|e| anyhow::anyhow!("Plotters Error: {}", e))?;

        root.present().map_err(|e| anyhow::anyhow!("Plotters Error: {}", e))?;

        Ok(format!("Successfully generated analytical chart! Saved natively to: {}", path))
    }
}

/// 📊 Linux GPU Usage Probe
/// Tries to find GPU usage % across different vendors (Intel, AMD, Nvidia)
#[cfg(target_os = "linux")]
pub fn get_linux_gpu_usage() -> i32 {
    // 1. Try NVIDIA via NVML if available
    if let Ok(nvml) = nvml_wrapper::Nvml::init() {
        if let Ok(device) = nvml.device_by_index(0) {
            if let Ok(util) = device.utilization_rates() {
                return util.gpu as i32;
            }
        }
    }

    // 2. Iterate through potential DRM cards (Intel, AMD)
    for card in 0..3 {
        let card_paths = [
            format!("/sys/class/drm/card{}/device", card),
            format!("/sys/class/drm/card{}", card),
        ];

        for base in card_paths {
            // A. Try direct usage counters (AMD, some newer Intel)
            if let Ok(content) = std::fs::read_to_string(format!("{}/gpu_busy_percent", base)) {
                if let Ok(val) = content.trim().parse::<i32>() {
                    return val;
                }
            }

            // B. Intel Frequency-based Proxy
            // If we can't get usage%, the ratio of current frequency vs max frequency is an excellent proxy for load.
            let cur_f = std::fs::read_to_string(format!("{}/gt_cur_freq_mhz", base));
            let max_f = std::fs::read_to_string(format!("{}/gt_max_freq_mhz", base));
            let min_f = std::fs::read_to_string(format!("{}/gt_min_freq_mhz", base));

            if let (Ok(cur), Ok(max), Ok(min)) = (cur_f, max_f, min_f) {
                let c_v = cur.trim().parse::<f32>().unwrap_or(0.0);
                let m_v = max.trim().parse::<f32>().unwrap_or(1.0);
                let n_v = min.trim().parse::<f32>().unwrap_or(0.0);

                if m_v > n_v {
                    let usage = ((c_v - n_v) / (m_v - n_v)) * 100.0;
                    return usage.clamp(0.0, 100.0) as i32;
                }
            }
        }
    }

    // 3. Last resort: status-based indicator
    if let Ok(status) = std::fs::read_to_string("/sys/class/drm/card0/device/power/runtime_status") {
        if status.trim() == "active" {
            return 5; 
        }
    }

    0
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn get_linux_gpu_usage() -> i32 { 0 }
