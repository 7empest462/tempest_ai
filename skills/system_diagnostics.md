---
name: system_diagnostics
description: Comprehensive system health check and performance diagnostics
---
## Steps
1. Gather system info:
   - `uname -a` — kernel
   - `cat /etc/os-release` — distro
   - `uptime` — load and uptime
   - `free -h` — memory
   - `df -h` — disk space
   - `lscpu` — CPU info

2. Check resource usage:
   - `top -bn1 | head -20` — process overview
   - `ps aux --sort=-%mem | head -10` — top memory consumers
   - `ps aux --sort=-%cpu | head -10` — top CPU consumers
   - `iostat -x 1 3` — disk I/O (if available)

3. Network diagnostics:
   - `ss -tulpn` — open ports and listeners
   - `ip addr` — interfaces
   - `ping -c 3 8.8.8.8` — connectivity
   - `dig google.com` — DNS resolution

4. Check for problems:
   - `dmesg -T | tail -20` — kernel messages
   - `journalctl -p err --since "1 hour ago"` — recent errors
   - `systemctl --failed` — failed services

5. Present a summary report to the user

## Key Notes
- On macOS, use `vm_stat` instead of `free`, `diskutil` instead of `lsblk`
- On macOS, use `sysctl -n hw.ncpu` for CPU count
- Always check both memory AND swap — high swap usage indicates memory pressure
- Load average > number of CPU cores = system is overloaded
