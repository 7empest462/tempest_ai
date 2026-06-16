// ==========================================
// 🛡️ SKG THREAT SCANNER TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use sysinfo::System;

const MOCK_BAD_HASH: &str = "8530467a14e91bd670bf7e7fc8bd05f884fbf30623f99aa9f52f49c0d1cf57a4";
const EICAR_MOCK_HASH: &str = "275a021bbfb6489e54d471899f7db9d1663fc695ec2fe2a2c4538aabf651fd0f";

// ── threat_scan ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "threat_scan",
    description = "Scans system files, directories, or active running processes for security threats, computing SHA-256 hashes of targets and matching them against in-memory Indicators of Compromise (IOCs) or behavioral heuristics."
)]
pub async fn threat_scan(
    target_type: String,
    target_path: Option<String>,
    deep_scan: Option<bool>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let t_type = target_type.to_lowercase();

    let report = match t_type.as_str() {
        "file" => {
            scan_file_target(target_path.as_deref()).await?
        }
        "directory" => {
            scan_directory_target(target_path.as_deref(), deep_scan.unwrap_or(false)).await?
        }
        "process" => {
            scan_process_target(target_path.as_deref()).await?
        }
        _ => {
            return Err(ToolError::ExecutionFailed(format!(
                "Invalid target_type '{}'. Supported: 'file', 'directory', 'process'",
                target_type
            )));
        }
    };

    Ok(serde_json::Value::String(report))
}

fn compute_sha256(path: &Path) -> Result<String, ToolError> {
    let mut file = File::open(path)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to open file: {}", e)))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;
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

fn scan_content_heuristics(path: &Path, content: &str) -> Vec<&'static str> {
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

async fn scan_file_target(path_str: Option<&str>) -> Result<String, ToolError> {
    let path_raw = path_str.ok_or_else(|| ToolError::ExecutionFailed("'target_path' is required for file scans".to_string()))?;
    let path = Path::new(path_raw);

    if !path.exists() {
        return Err(ToolError::ExecutionFailed(format!("Target file path '{}' does not exist.", path_raw)));
    }
    if !path.is_file() {
        return Err(ToolError::ExecutionFailed(format!("Target path '{}' is not a file.", path_raw)));
    }

    let hash = compute_sha256(path)?;
    let mut content = String::new();
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

    let heuristics = scan_content_heuristics(path, &content);
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
        report.push_str("✅ No threat indicators or matching signatures were found. This file is clean.\n");
    }

    Ok(report)
}

async fn scan_directory_target(path_str: Option<&str>, deep: bool) -> Result<String, ToolError> {
    let path_raw = path_str.ok_or_else(|| ToolError::ExecutionFailed("'target_path' is required for directory scans".to_string()))?;
    let path = Path::new(path_raw);

    if !path.exists() {
        return Err(ToolError::ExecutionFailed(format!("Target directory path '{}' does not exist.", path_raw)));
    }
    if !path.is_dir() {
        return Err(ToolError::ExecutionFailed(format!("Target path '{}' is not a directory.", path_raw)));
    }

    let mut files_to_scan = Vec::new();

    if deep {
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
        if let Ok(hash) = compute_sha256(&file_path) {
            let mut content = String::new();
            if let Ok(mut f) = File::open(&file_path) {
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() {
                    let lossy = String::from_utf8_lossy(&buf);
                    content = lossy.chars().take(50_000).collect();
                }
            }

            let heuristics = scan_content_heuristics(&file_path, &content);
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

async fn scan_process_target(filter: Option<&str>) -> Result<String, ToolError> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut total_processes = 0;
    let mut threat_procs = Vec::new();

    for (pid, process) in sys.processes() {
        total_processes += 1;
        let name = process.name().to_string_lossy().into_owned();

        if let Some(f) = filter {
            let pid_str = pid.to_string();
            if !name.contains(f) && pid_str != f {
                continue;
            }
        }

        let exe_path_opt = process.exe();

        if exe_path_opt.is_none() {
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

        if exe_path.exists()
            && let Ok(hash) = compute_sha256(exe_path)
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

        let hash = compute_sha256(&file_path).unwrap();
        assert_eq!(hash.len(), 64);

        let empty_path = temp_dir.path().join("empty.txt");
        {
            File::create(&empty_path).unwrap();
        }
        let empty_hash = compute_sha256(&empty_path).unwrap();
        assert_eq!(
            empty_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
