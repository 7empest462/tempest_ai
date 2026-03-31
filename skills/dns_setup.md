---
name: dns_setup
description: Configure and troubleshoot DNS servers (Unbound, Pi-hole, systemd-resolved)
---
## Unbound Setup
1. Install: `sudo apt install unbound`
2. Configure `/etc/unbound/unbound.conf`:
   - Set `interface: 0.0.0.0` for network-wide access
   - Set `access-control: 192.168.0.0/16 allow`
   - Enable DNSSEC: `auto-trust-anchor-file`
   - Set caching: `msg-cache-size: 64m`, `rrset-cache-size: 128m`
3. Test: `dig @127.0.0.1 google.com`
4. Check status: `sudo systemctl status unbound`

## Troubleshooting Slow DNS
1. Check resolution time: `dig @<server> example.com | grep "Query time"`
2. If >200ms, check:
   - Cache hit ratio: `unbound-control stats_noreset | grep cache`
   - Forwarding vs recursive: forwarding is faster for most home use
   - Prefetching: add `prefetch: yes` and `prefetch-key: yes`
3. Test with: `time nslookup google.com <server_ip>`

## Pi-hole Integration
- Point Pi-hole upstream DNS to Unbound: `127.0.0.1#5335`
- Unbound listens on port 5335 for Pi-hole queries
- Pi-hole handles ad blocking, Unbound handles resolution

## Key Notes
- `systemd-resolved` conflicts with Unbound on port 53 — disable it:
  `sudo systemctl disable systemd-resolved && sudo systemctl stop systemd-resolved`
- Always test DNS changes from a CLIENT device, not the server itself
- Flush cache after config changes: `unbound-control reload`
