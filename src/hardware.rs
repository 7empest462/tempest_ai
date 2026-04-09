use miette::{Result, miette, IntoDiagnostic};
use serde_json::Value;
use crate::tools::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct LinuxProcessAnalyzerArgs {
    /// The target Process ID to analyze.
    pub pid: i32,
}

#[allow(dead_code)]
pub struct LinuxProcessAnalyzerTool;

#[async_trait::async_trait]
impl AgentTool for LinuxProcessAnalyzerTool {
    fn name(&self) -> &'static str { "linux_process_analyzer" }
    fn description(&self) -> &'static str { 
        "CRITICAL: NVIDIA ONLY. Read detailed process memory maps, IO counters, and thread counts directly from the Linux kernel. DO NOT USE ON MACOS. If you are on a Mac, use `system_diagnostic_scan` to see GPU stats."
    }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<LinuxProcessAnalyzerArgs>();
        
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
        let typed_args: LinuxProcessAnalyzerArgs = serde_json::from_value(args.clone())
            .map_err(|e| miette!("Invalid arguments for linux_process_analyzer: {}", e))?;
        
        #[cfg(target_os = "linux")]
        {
            let pid = typed_args.pid;
            let process = procfs::process::Process::new(pid).into_diagnostic()?;
            
            let stat = process.stat().into_diagnostic()?;
            let io = process.io().into_diagnostic()?;
            let cmdline = process.cmdline().into_diagnostic()?.join(" ");

            let mut out = format!("Process [{}] - {}\n", pid, cmdline);
            out.push_str(&format!("State: {:?}\n", stat.state));
            out.push_str(&format!("Threads: {}\n", stat.num_threads));
            out.push_str(&format!("RSS Memory: {} bytes\n", stat.rss_bytes().get()));
            out.push_str(&format!("Char Read / Write: {} / {} bytes\n", io.rchar, io.wchar));
            
            Ok(out)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = typed_args;
            Ok("Error: The linux_process_analyzer tool relies on the pristine Linux procfs kernel mapping. You are currently running on macOS (Darwin). Use `system_info` or `run_command` with macOS specific polling instead.".to_string())
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct GpuDiagnosticsArgs {
    /// Optional GPU ID (default 0).
    pub gpu_id: Option<u32>,
}

#[allow(dead_code)]
pub struct GpuDiagnosticsTool;

#[async_trait::async_trait]
impl AgentTool for GpuDiagnosticsTool {
    fn name(&self) -> &'static str { "gpu_diagnostics" }
    fn description(&self) -> &'static str { "Read Nvidia GPU telemetry (temperature, clock speeds, active instances) natively." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<GpuDiagnosticsArgs>();
        
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
        let typed_args: GpuDiagnosticsArgs = serde_json::from_value(args.clone())
            .unwrap_or(GpuDiagnosticsArgs { gpu_id: Some(0) });
            
        #[cfg(target_os = "linux")]
        {
            let gpu_id = typed_args.gpu_id.unwrap_or(0);
            
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
            let _ = typed_args;
            Ok("Error: The gpu_diagnostics tool maps exclusively to the Nvidia Hardware Management Library. Your current host is an Apple Silicon Mac without an Nvidia GPU.".to_string())
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct TelemetryChartArgs {
    /// Chart Title
    pub title: String,
    /// X-axis Label
    pub x_label: String,
    /// Y-axis Label
    pub y_label: String,
    /// Name of the line series
    pub series_name: String,
    /// An array of precise [X, Y] arrays (e.g. [[1, 5], [2, 10], [3, 20]]). CRITICAL: MUST BE RAW FLOATS. DO NOT use expressions like Math.sin()
    pub data_points: Vec<Vec<f64>>,
}

#[allow(dead_code)]
pub struct TelemetryChartTool;

#[async_trait::async_trait]
impl AgentTool for TelemetryChartTool {
    fn name(&self) -> &'static str { "generate_telemetry_chart" }
    fn description(&self) -> &'static str { "Generate a high-quality .png line-chart from arrays of X/Y data points. Useful for graphing CPU hogs, memory over time, or network spikes." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TelemetryChartArgs>();
        
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
        let typed_args: TelemetryChartArgs = serde_json::from_value(args.clone())
            .map_err(|e| miette!("Invalid parameters for chart tool: {}", e))?;
            
        let title = typed_args.title.as_str();
        let x_label = typed_args.x_label.as_str();
        let y_label = typed_args.y_label.as_str();
        let series_name = typed_args.series_name.as_str();
        
        let mut points: Vec<(f64, f64)> = Vec::new();
        for p in typed_args.data_points {
            if p.len() == 2 {
                points.push((p[0], p[1]));
            }
        }

        if points.is_empty() {
            return Ok("Error: No valid data points provided.".to_string());
        }

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
        
        let x_padding = (max_x - min_x) * 0.05;
        let y_padding = (max_y - min_y) * 0.05;

        min_x -= x_padding;
        max_x += x_padding;
        min_y -= y_padding;
        max_y += y_padding;
        
        if min_x == max_x { max_x += 1.0; min_x -= 1.0; }
        if min_y == max_y { max_y += 1.0; min_y -= 1.0; }

        let path = format!("/tmp/tempest_chart_{}.png", format!("{}", chrono::Local::now().format("%H%M%S")));
        
        use plotters::prelude::*;
        let root = BitMapBackend::new(&path, (800, 600)).into_drawing_area();
        root.fill(&WHITE).map_err(|e| miette!("Plotters Error: {}", e))?;

        let mut chart = ChartBuilder::on(&root)
            .caption(title, ("sans-serif", 30).into_font())
            .margin(20)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(min_x..max_x, min_y..max_y)
            .map_err(|e| miette!("Plotters Error: {}", e))?;

        chart.configure_mesh()
            .x_desc(x_label)
            .y_desc(y_label)
            .draw()
            .map_err(|e| miette!("Plotters Error: {}", e))?;

        chart.draw_series(LineSeries::new(points, &RED))
            .map_err(|e| miette!("Plotters Error: {}", e))?
            .label(series_name)
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

        chart.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()
            .map_err(|e| miette!("Plotters Error: {}", e))?;

        root.present().map_err(|e| miette!("Plotters Error: {}", e))?;

        Ok(format!("Successfully generated analytical chart! Saved natively to: {}", path))
    }
}

#[cfg(target_os = "linux")]
pub fn get_linux_gpu_usage() -> i32 {
    if let Ok(nvml) = nvml_wrapper::Nvml::init() {
        if let Ok(device) = nvml.device_by_index(0) {
            if let Ok(util) = device.utilization_rates() {
                return util.gpu as i32;
            }
        }
    }

    for card in 0..3 {
        let card_paths = [
            format!("/sys/class/drm/card{}/device", card),
            format!("/sys/class/drm/card{}", card),
        ];

        for base in card_paths {
            if let Ok(content) = std::fs::read_to_string(format!("{}/gpu_busy_percent", base)) {
                if let Ok(val) = content.trim().parse::<i32>() {
                    return val;
                }
            }

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
