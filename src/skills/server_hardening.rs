pub const NAME: &str = "server_hardening";
pub const DESCRIPTION: &str = "Harden a Linux server with firewall, SSH, and security best practices";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Audit current state:
   - uname -a — kernel version
   - cat /etc/os-release — distro
   - ss -tulpn — open ports
   - who — active sessions
   - sudo ufw status or sudo iptables -L — firewall

2. SSH Hardening (/etc/ssh/sshd_config):
   - PermitRootLogin no
   - PasswordAuthentication no (use keys only)
   - Port 2222 (change from default 22)
   - MaxAuthTries 3
   - AllowUsers <username>
   - Restart: sudo systemctl restart sshd

3. Firewall (UFW):
   - sudo ufw default deny incoming
   - sudo ufw default allow outgoing
   - sudo ufw allow <ssh_port>/tcp
   - sudo ufw allow 80/tcp (if web server)
   - sudo ufw allow 443/tcp (if HTTPS)
   - sudo ufw enable

4. Automatic Updates:
   - sudo apt install unattended-upgrades
   - sudo dpkg-reconfigure unattended-upgrades

5. Fail2ban:
   - sudo apt install fail2ban
   - Create /etc/fail2ban/jail.local with SSH jail

## Key Notes
- ALWAYS verify SSH access works with new settings BEFORE closing your current session
- Keep a backup terminal open during SSH config changes
- Test firewall rules: sudo ufw status numbered
"#;
