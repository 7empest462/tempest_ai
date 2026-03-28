use anyhow::Result;
use serde_json::Value;
use crate::tools::AgentTool;

pub struct AdvancedSystemOracleTool;

#[async_trait::async_trait]
impl AgentTool for AdvancedSystemOracleTool {
    fn name(&self) -> &'static str { "system_oracle_3d" }
    fn description(&self) -> &'static str { "Perform a deep 3D topological sweep of the host environment. Returns exhaustive details on CPU layouts, physical memory, swap, mapped disks, and component thermals." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
    async fn execute(&self, _args: &Value, _agent_content: &str) -> Result<String> {
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
        let components = Components::new_with_refreshed_list();
        if components.is_empty() {
            out.push_str("(No thermal sensors exposed to user space)\n");
        }
        for comp in &components {
            out.push_str(&format!("- {}: {:?}°C (Max: {:?}°C)\n", comp.label(), comp.temperature(), comp.max()));
        }
        
        out.push_str("\n🕸️  NETWORKS\n");
        let networks = Networks::new_with_refreshed_list();
        for (name, data) in &networks {
            out.push_str(&format!("- {}: MAC {} | Tx: {}B, Rx: {}B\n", name, data.mac_address(), data.total_transmitted(), data.total_received()));
        }

        Ok(out)
    }
}

pub struct KernelDiagnosticTool;

#[async_trait::async_trait]
impl AgentTool for KernelDiagnosticTool {
    fn name(&self) -> &'static str { "kernel_sysctl" }
    fn description(&self) -> &'static str { "Query Unix/macOS deep kernel configurations via sysctl (e.g. read 'kern.maxfiles' or 'hw.ncpu')." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The sysctl key to read (e.g., 'hw.model', 'kern.boottime', 'net.inet.tcp.keepinit')." }
            },
            "required": ["key"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let key = args.get("key").and_then(|k| k.as_str()).unwrap_or("");
        
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

pub struct NetworkSnifferTool;

#[async_trait::async_trait]
impl AgentTool for NetworkSnifferTool {
    fn name(&self) -> &'static str { "network_sniffer" }
    fn description(&self) -> &'static str { "Analyze raw network topology and intercept raw packets using pnet. Provide 'action': 'list_interfaces' to see adapters, or 'sniff' and 'interface_name' to capture 5 packets (REQUIRES SUDO / ROOT PERMISSIONS)." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list_interfaces", "sniff"], "description": "Action to perform" },
                "interface_name": { "type": "string", "description": "Interface name to sniff (e.g. 'en0' or 'eth0'). Ignored for list_interfaces." }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        use pnet::datalink::{self, NetworkInterface};

        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("list_interfaces");

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
            let iface_name = args.get("interface_name").and_then(|i| i.as_str()).unwrap_or("");
            
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
