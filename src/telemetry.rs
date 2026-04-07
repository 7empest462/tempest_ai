use anyhow::Result;
use serde_json::Value;
use crate::tools::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct AdvancedSystemOracleArgs {}

pub struct AdvancedSystemOracleTool;

#[async_trait::async_trait]
impl AgentTool for AdvancedSystemOracleTool {
    fn name(&self) -> &'static str { "system_oracle_3d" }
    fn description(&self) -> &'static str { "Perform a deep 3D topological sweep of the host environment. Returns exhaustive details on CPU layouts, physical memory, swap, mapped disks, and component thermals." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<AdvancedSystemOracleArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }
    async fn execute(&self, _args: &Value, _context: ToolContext) -> Result<String> {
        use sysinfo::{System, Disks, Networks, Components};
        
        let mut sys = System::new_all();
        sys.refresh_all();
        
        let mut out = String::new();
        out.push_str("=== 3D System Topology ===\n\n");
        
        out.push_str("🖥️  CPU & SYSTEM\n");
        out.push_str(&format!("OS: {} {}\n", System::name().unwrap_or_default(), System::os_version().unwrap_or_default()));
        out.push_str(&format!("Host: {}\n", System::host_name().unwrap_or_default()));
        out.push_str(&format!("Physical Cores: {:?}\n", System::physical_core_count()));
        out.push_str(&format!("Logical Threads: {}\n", sys.cpus().len()));
        
        out.push_str("\n🧠 MEMORY\n");
        out.push_str(&format!("RAM: {} / {} MB\n", sys.used_memory() / 1024 / 1024, sys.total_memory() / 1024 / 1024));
        out.push_str(&format!("SWAP: {} / {} MB\n", sys.used_swap() / 1024 / 1024, sys.total_swap() / 1024 / 1024));
        
        out.push_str("\n💾 DISKS\n");
        let disks = Disks::new_with_refreshed_list();
        for disk in &disks {
            out.push_str(&format!("- {:?} [{:?}] | Mounted at: {:?} | {} / {} GB\n", 
                disk.name(), disk.kind(), disk.mount_point(),
                (disk.total_space() - disk.available_space()) / 1_000_000_000,
                disk.total_space() / 1_000_000_000
            ));
        }

        out.push_str("\n🔥 COMPONENTS & THERMALS\n");
        #[cfg(target_os = "linux")]
        {
            // Direct sysfs/hwmon reading for Linux
            let entries = std::fs::read_dir("/sys/class/thermal").ok();
            let mut found = false;
            if let Some(dirs) = entries {
                for dir in dirs.flatten() {
                    let path = dir.path();
                    let type_path = path.join("type");
                    let temp_path = path.join("temp");
                    
                    if let (Ok(t), Ok(v)) = (std::fs::read_to_string(type_path), std::fs::read_to_string(temp_path)) {
                        let temp_f = v.trim().parse::<f32>().unwrap_or(0.0) / 1000.0;
                        out.push_str(&format!("- {}: {:.2}°C\n", t.trim(), temp_f));
                        found = true;
                    }
                }
            }
            if !found {
                out.push_str("(No sysfs thermal data found. Reverting to basic sensors...)\n");
                let components = Components::new_with_refreshed_list();
                for comp in &components {
                    out.push_str(&format!("- {}: {:?}°C\n", comp.label(), comp.temperature()));
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let components = Components::new_with_refreshed_list();
            if components.is_empty() {
                out.push_str("(No thermal sensors exposed to user space)\n");
            }
            for comp in &components {
                out.push_str(&format!("- {}: {:?}°C (Max: {:?}°C)\n", comp.label(), comp.temperature(), comp.max()));
            }
        }
        
        out.push_str("\n🕸️  NETWORKS\n");
        let networks = Networks::new_with_refreshed_list();
        for (name, data) in &networks {
            out.push_str(&format!("- {}: MAC {} | Tx: {}B, Rx: {}B\n", name, data.mac_address(), data.total_transmitted(), data.total_received()));
        }

        Ok(out)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct KernelDiagnosticArgs {
    /// The sysctl key to read (e.g., 'hw.model', 'kern.boottime', 'net.inet.tcp.keepinit').
    pub key: String,
}

pub struct KernelDiagnosticTool;

#[async_trait::async_trait]
impl AgentTool for KernelDiagnosticTool {
    fn name(&self) -> &'static str { "kernel_sysctl" }
    fn description(&self) -> &'static str { "Query Unix/macOS deep kernel configurations via sysctl (e.g. read 'kern.maxfiles' or 'hw.ncpu')." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<KernelDiagnosticArgs>();
        
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
        let typed_args: KernelDiagnosticArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow::anyhow!("Invalid parameters: {}", e))?;
        let key = typed_args.key.as_str();
        
        #[cfg(unix)]
        {
            use sysctl::Sysctl;
            let ctl = match sysctl::Ctl::new(key) {
                Ok(c) => c,
                Err(e) => return Ok(format!("Failed to locate sysctl key '{}': {}", key, e)),
            };
            
            let val = match ctl.value() {
                Ok(v) => format!("{:?}", v),
                Err(e) => format!("Error reading value: {}", e),
            };
            
            let desc = ctl.description().unwrap_or_else(|_| "No description".to_string());
            
            Ok(format!("Sysctl: {}\nDescription: {}\nValue: {}", key, desc, val))
        }
        #[cfg(not(unix))]
        {
            Ok("Error: sysctl is only available on Unix-like operating systems.".to_string())
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct NetworkSnifferArgs {
    /// Action to perform: 'list_interfaces' or 'sniff'
    pub action: String,
    /// Interface name to sniff (e.g. 'en0' or 'eth0'). Ignored for list_interfaces.
    pub interface_name: Option<String>,
}

pub struct NetworkSnifferTool;

#[async_trait::async_trait]
impl AgentTool for NetworkSnifferTool {
    fn name(&self) -> &'static str { "network_sniffer" }
    fn description(&self) -> &'static str { "Analyze raw network topology and intercept raw packets using pnet. Provide 'action': 'list_interfaces' to see adapters, or 'sniff' and 'interface_name' to capture 5 packets (REQUIRES SUDO / ROOT PERMISSIONS)." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<NetworkSnifferArgs>();
        
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
        use pnet::datalink::{self, NetworkInterface};

        let typed_args: NetworkSnifferArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow::anyhow!("Invalid parameters: {}", e))?;
        let action = typed_args.action.as_str();

        let interfaces = datalink::interfaces();

        if action == "list_interfaces" {
            let mut out = String::from("=== Active Network Interfaces ===\n");
            for iface in &interfaces {
                out.push_str(&format!("{} [{}]\n", iface.name, iface.description));
                if let Some(mac) = iface.mac {
                    out.push_str(&format!("  MAC: {}\n", mac));
                }
                for ip in &iface.ips {
                    out.push_str(&format!("  IP: {}\n", ip.ip()));
                }
            }
            return Ok(out);
        }

        if action == "sniff" {
            let iface_name = typed_args.interface_name.as_deref().unwrap_or("");
            
            let interface_match = interfaces.into_iter()
                .find(|iface: &NetworkInterface| iface.name == iface_name);

            let interface = match interface_match {
                Some(i) => i,
                None => return Ok(format!("Error: Could not find interface '{}'. Use list_interfaces first.", iface_name)),
            };

            // Trying to open a raw socket requires root
            let (_tx, mut rx) = match datalink::channel(&interface, Default::default()) {
                Ok(datalink::Channel::Ethernet(tx, rx)) => (tx, rx),
                Ok(_) => return Ok("Error: Unhandled channel type".to_string()),
                Err(e) => return Ok(format!("CRITICAL CAPTURE ERROR: Failed to create datalink channel: {}. (Are you running the agent with sudo/root permissions? Packet sniffing requires elevated privileges on macOS/Linux.)", e)),
            };

            let mut out = format!("Capturing 5 raw Ethernet frames on interface '{}'...\n", iface_name);
            
            // Capture just 5 packets to avoid blocking the AI forever
            for i in 1..=5 {
                match rx.next() {
                    Ok(packet) => {
                        let pkt = pnet::packet::ethernet::EthernetPacket::new(packet).unwrap();
                        out.push_str(&format!("[Packet {}] {} -> {} (ethertype: {:?}) | Size: {} bytes\n", 
                            i, pkt.get_source(), pkt.get_destination(), pkt.get_ethertype(), packet.len()));
                    },
                    Err(e) => {
                        out.push_str(&format!("Error reading packet: {}\n", e));
                        break;
                    }
                }
            }
            return Ok(out);
        }

        Ok("Error: Unknown action.".to_string())
    }
}
