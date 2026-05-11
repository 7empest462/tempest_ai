// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.

use crate::tui::AgentEvent;
use sysinfo::System;
use tokio::sync::mpsc::Sender;
use std::time::Duration;
use tokio::time::interval;

pub struct TelemetryCollector {
    sys: System,
    event_tx: Sender<AgentEvent>,
}

impl TelemetryCollector {
    pub fn new(event_tx: Sender<AgentEvent>) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self { sys, event_tx }
    }

    pub async fn run(mut self) {
        let mut interval = interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            self.sys.refresh_all();
            
            // In sysinfo 0.30+, global_cpu_usage() returns f32 directly
            let cpu_usage = self.sys.global_cpu_usage() as u64;
            let total_ram = self.sys.total_memory();
            let used_ram = self.sys.used_memory();
            let ram_usage_pct = if total_ram > 0 {
                (used_ram * 100 / total_ram) as u64
            } else {
                0
            };

            // Simplified GPU metric for now
            let gpu_usage = None; 

            let _ = self.event_tx.try_send(AgentEvent::TelemetryMetrics { 
                cpu: Some(cpu_usage), 
                gpu: gpu_usage, 
                ram: Some(ram_usage_pct),
                tps: None 
            });
        }
    }
}
