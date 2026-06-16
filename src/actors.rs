use parking_lot::Mutex;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use regex::Regex;
use std::sync::Arc;
use sysinfo::{Components, Networks, System};

pub enum TelemetryMessage {
    Tick,
    GetLatestTelemetry(tokio::sync::oneshot::Sender<String>),
}

pub struct TelemetryArgs {
    pub agent_tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>,
    pub shared_telemetry: Arc<Mutex<String>>,
    pub mode: crate::inference::AgentMode,
}

pub struct TelemetryActor;

pub struct TelemetryState {
    sys: System,
    networks: Networks,
    components: Components,
    vram_re: Regex,
    last_update: String,
    agent_tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>,
    shared_telemetry: Arc<Mutex<String>>,
    mode: crate::inference::AgentMode,
}

impl Actor for TelemetryActor {
    type Msg = TelemetryMessage;
    type State = TelemetryState;
    type Arguments = TelemetryArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let sys = System::new_all();
        let networks = Networks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();
        let vram_re =
            Regex::new(r#""(?:Alloc|In use) system memory(?:\s*\(driver\))?"\s*=\s*(\d+)"#)
                .unwrap();

        // Spawn a periodic tick timer that triggers Tick messages to this actor
        let myself_clone = myself.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if myself_clone.cast(TelemetryMessage::Tick).is_err() {
                    break;
                }
            }
        });

        Ok(TelemetryState {
            sys,
            networks,
            components,
            vram_re,
            last_update: String::new(),
            agent_tx: args.agent_tx,
            shared_telemetry: args.shared_telemetry,
            mode: args.mode,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            TelemetryMessage::Tick => {
                state.sys.refresh_all();
                state.networks.refresh(true);
                state.components.refresh(true);

                let mut ollama_mem_bytes: u64 = 0;
                let mut tempest_mem_bytes: u64 = 0;
                let mut lmstudio_mem_bytes: u64 = 0;
                let self_pid = std::process::id() as i32;
                for process in state.sys.processes().values() {
                    let name = process.name().to_string_lossy().to_lowercase();
                    let exe = process
                        .exe()
                        .map(|p| p.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let pid = process.pid().as_u32() as i32;

                    let mut mem = process.memory();
                    #[cfg(target_os = "macos")]
                    {
                        if let Some(meta) = tempest_monitor::macos_helper::get_process_metadata(pid) {
                            mem += meta.compressed;
                        }
                    }

                    if name.contains("tempest_ai")
                        || name.contains("tempest-ai")
                        || pid == self_pid
                    {
                        tempest_mem_bytes += mem;
                    } else if name.contains("ollama") || name.contains("llama-server") {
                        ollama_mem_bytes += mem;
                    } else if name.contains("lm studio")
                        || name.contains("lmstudio")
                        || exe.contains(".lmstudio")
                    {
                        lmstudio_mem_bytes += mem;
                    }
                }

                #[cfg(target_os = "macos")]
                let mac_gpu = tempest_monitor::macos_helper::get_macos_gpu_info(false);

                let ai_ram_mb = match state.mode {
                    crate::inference::AgentMode::MLX => {
                        let vram_mb: u64 = {
                            #[cfg(target_os = "macos")]
                            {
                                if let Ok(output) = std::process::Command::new("ioreg")
                                    .args(["-r", "-d", "1", "-c", "AGXAccelerator"])
                                    .output()
                                {
                                    let s = String::from_utf8_lossy(&output.stdout);
                                    state
                                        .vram_re
                                        .captures_iter(&s)
                                        .filter_map(|caps| caps.get(1))
                                        .map(|m| {
                                            m.as_str().parse::<u64>().unwrap_or(0) / 1024 / 1024
                                        })
                                        .sum()
                                } else {
                                    0
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            0
                        };
                        (tempest_mem_bytes / 1024 / 1024) + vram_mb
                    }
                    crate::inference::AgentMode::Ollama => {
                        (ollama_mem_bytes + tempest_mem_bytes) / 1024 / 1024
                    }
                    crate::inference::AgentMode::Bridge => tempest_mem_bytes / 1024 / 1024,
                    crate::inference::AgentMode::LMStudio => {
                        (lmstudio_mem_bytes + tempest_mem_bytes) / 1024 / 1024
                    }
                    crate::inference::AgentMode::Gemini => tempest_mem_bytes / 1024 / 1024,
                    crate::inference::AgentMode::Kalosm => tempest_mem_bytes / 1024 / 1024,
                };

                let engine_label = match state.mode {
                    crate::inference::AgentMode::MLX => "(Native Engine)",
                    crate::inference::AgentMode::Ollama => "(Ollama)",
                    crate::inference::AgentMode::Bridge => "(Bridge)",
                    crate::inference::AgentMode::LMStudio => "(LM Studio)",
                    crate::inference::AgentMode::Gemini => "(Google Gemini)",
                    crate::inference::AgentMode::Kalosm => "(Kalosm Native)",
                };

                let gpu_load = {
                    #[cfg(target_os = "macos")]
                    {
                        mac_gpu.usage_pct as i32
                    }
                    #[cfg(target_os = "linux")]
                    {
                        crate::hardware::get_linux_gpu_usage()
                    }
                    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                    {
                        0
                    }
                };

                let cpus = state.sys.cpus();
                let mut total_cpu = 0.0;
                for cpu in cpus {
                    total_cpu += cpu.cpu_usage();
                }
                let avg_cpu = if !cpus.is_empty() {
                    total_cpu / cpus.len() as f32
                } else {
                    0.0
                };

                let mem_seg = tempest_monitor::system_helper::get_memory_segments(&state.sys);
                let total_mb = mem_seg.total / 1024 / 1024;
                let used_mb = (mem_seg.active + mem_seg.wired) / 1024 / 1024;
                let active_mb = mem_seg.active / 1024 / 1024;
                let wired_mb = mem_seg.wired / 1024 / 1024;
                let cache_mb = mem_seg.cache / 1024 / 1024;
                let free_mb = mem_seg.free / 1024 / 1024;

                let mem_perc = if mem_seg.total > 0 {
                    ((mem_seg.active + mem_seg.wired) as f32 / mem_seg.total as f32) * 100.0
                } else {
                    0.0
                };

                let used_swap = state.sys.used_swap() / 1024 / 1024;
                let total_swap = state.sys.total_swap() / 1024 / 1024;
                let swap_perc = if total_swap > 0 {
                    (used_swap as f32 / total_swap as f32) * 100.0
                } else {
                    0.0
                };

                let mut total_rx = 0;
                let mut total_tx = 0;
                for (interface_name, data) in &state.networks {
                    if interface_name == "en0"
                        || interface_name.starts_with("eth")
                        || interface_name.starts_with("wlan")
                    {
                        total_rx += data.received();
                        total_tx += data.transmitted();
                    }
                }

                let mut max_temp = 0.0;
                let mut sum_temp = 0.0;
                let mut count_temp = 0;
                for comp in &state.components {
                    if let Some(mut temp) = comp.temperature().filter(|&t| t > 0.0) {
                        if temp > 500.0 {
                            temp /= 1000.0;
                        }
                        if temp > 150.0 {
                            continue;
                        }
                        if temp > max_temp {
                            max_temp = temp;
                        }
                        sum_temp += temp;
                        count_temp += 1;
                    }
                }
                let avg_temp = if count_temp > 0 {
                    sum_temp / count_temp as f32
                } else {
                    0.0
                };

                let uptime = System::uptime();
                let hours = uptime / 3600;
                let minutes = (uptime % 3600) / 60;
                let secs = uptime % 60;
                let proc_count = state.sys.processes().len();

                #[cfg(target_os = "macos")]
                let gpu_freq_str = if let Some(f) = mac_gpu.gpu_freq_mhz {
                    format!(" @ {:.0} MHz", f)
                } else {
                    "".to_string()
                };
                #[cfg(not(target_os = "macos"))]
                let gpu_freq_str = "".to_string();

                #[cfg(target_os = "macos")]
                let ane_power_str = if let Some(p) = mac_gpu.ane_power_mw {
                    format!("\n\n🧠 ANE POWER      : {:.0} mW (Neural Engine)", p)
                } else {
                    "".to_string()
                };
                #[cfg(not(target_os = "macos"))]
                let ane_power_str = "".to_string();

                let mut update_str = format!(
                    "🔥 CPU LOAD       : {:.1}% ({} Cores)

🚀 MEMORY ALLOC   : {}/{} MB ({:.1}%) [Active: {}MB, Wired: {}MB, Cache: {}MB, Free: {}MB]

🤖 AI RAM USE     : {} MB {}

🎨 GPU LOAD       : {}% (Graphics){}{}

💾 SWAP CACHE     : {}/{} MB ({:.1}%)

----------------------------------

🛰️ NETWORK [en0]    : {} B ▼ | {} B ▲

🌡️ AVG THERMALS   : {:.1} °C (Max: {:.1} °C)

⚙️ ACTIVE PROCS   : {}

⏱️ CORE UPTIME    : {}h {}m {}s",
                    avg_cpu,
                    cpus.len(),
                    used_mb,
                    total_mb,
                    mem_perc,
                    active_mb,
                    wired_mb,
                    cache_mb,
                    free_mb,
                    ai_ram_mb,
                    engine_label,
                    gpu_load,
                    gpu_freq_str,
                    ane_power_str,
                    used_swap,
                    total_swap,
                    swap_perc,
                    total_rx,
                    total_tx,
                    avg_temp,
                    max_temp,
                    proc_count,
                    hours,
                    minutes,
                    secs
                );

                #[cfg(target_os = "linux")]
                if tempest_monitor::linux_helper::is_steamos() {
                    update_str.push_str("\n\n🩺 STEAMOS CHECK : MATCHED");
                }
                update_str.push_str("\n\n----------------------------------");

                state.last_update = update_str.clone();

                let _ = state
                    .agent_tx
                    .send(crate::tui::AgentEvent::SystemUpdate(update_str.clone()))
                    .await;

                let _ = state
                    .agent_tx
                    .send(crate::tui::AgentEvent::TelemetryMetrics {
                        cpu: Some((avg_cpu * 100.0) as u64),
                        gpu: Some(gpu_load as u64 * 100),
                        ram: Some(mem_perc as u64),
                        tps: None,
                    })
                    .await;

                {
                    let mut lock = state.shared_telemetry.lock();
                    *lock = update_str;
                }
            }
            TelemetryMessage::GetLatestTelemetry(tx) => {
                let _ = tx.send(state.last_update.clone());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telemetry_actor() {
        let (agent_tx, mut agent_rx) = tokio::sync::mpsc::channel(100);
        let shared_telemetry = Arc::new(Mutex::new(String::new()));

        let (actor_ref, _actor_handle) = Actor::spawn(
            None,
            TelemetryActor,
            TelemetryArgs {
                agent_tx,
                shared_telemetry: shared_telemetry.clone(),
                mode: crate::inference::AgentMode::Bridge,
            },
        )
        .await
        .expect("Failed to spawn TelemetryActor");

        // Wait a short moment or send a manual tick
        actor_ref
            .cast(TelemetryMessage::Tick)
            .expect("Failed to send Tick");

        // Give the actor a moment to process the tick
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Check that the shared telemetry was updated
        {
            let lock = shared_telemetry.lock();
            assert!(lock.contains("CPU LOAD"));
            assert!(lock.contains("MEMORY ALLOC"));
        }

        // Test GetLatestTelemetry message
        let (tx, rx) = tokio::sync::oneshot::channel();
        actor_ref
            .cast(TelemetryMessage::GetLatestTelemetry(tx))
            .expect("Failed to send GetLatestTelemetry");
        let latest = rx.await.expect("Failed to receive latest telemetry");
        assert!(latest.contains("CPU LOAD"));

        // Test that agent_tx received events
        let mut got_system_update = false;
        let mut got_telemetry_metrics = false;
        while let Ok(event) = agent_rx.try_recv() {
            match event {
                crate::tui::AgentEvent::SystemUpdate(_) => got_system_update = true,
                crate::tui::AgentEvent::TelemetryMetrics { .. } => got_telemetry_metrics = true,
                _ => {}
            }
        }
        assert!(got_system_update, "Should receive SystemUpdate");
        assert!(got_telemetry_metrics, "Should receive TelemetryMetrics");

        // Cleanup
        actor_ref.stop(None);
    }
}
