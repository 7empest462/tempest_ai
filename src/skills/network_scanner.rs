pub const NAME: &str = "network_scanner";
pub const DESCRIPTION: &str = "Scan a local network for hosts, open ports, and running services";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Determine the local network range using run_command: ip route | grep default or ifconfig
2. Write a Python scanner using extract_and_write with:
   - socket for TCP port scanning
   - subprocess for ARP discovery (arp -a or ip neigh)
   - concurrent.futures.ThreadPoolExecutor for parallel scanning
   - Common ports: 22, 53, 80, 443, 8080, 8443, 3000, 5000, 8096, 9090, 3306, 5432
3. Make it executable with chmod
4. Run the scan and display results

## Key Notes
- Always use socket.settimeout(1) to prevent hanging on closed ports
- Use connect_ex() instead of connect() — returns 0 on success instead of raising
- Use ThreadPoolExecutor with max_workers=50 for speed without overwhelming the network
- Print results as a clean table: IP | Port | Service | Status
- Common service names: 22=SSH, 53=DNS, 80=HTTP, 443=HTTPS, 8096=Jellyfin, 9090=Prometheus
"#;
