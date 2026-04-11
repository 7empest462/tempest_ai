pub const NAME: &str = "systemd_service";
pub const DESCRIPTION: &str = "Create and manage a systemd service unit for Linux daemons";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Determine the binary path, working directory, and user to run as
2. Create the service unit file using extract_and_write at /etc/systemd/system/<name>.service
3. Reload systemd: sudo systemctl daemon-reload
4. Enable on boot: sudo systemctl enable <name>
5. Start the service: sudo systemctl start <name>
6. Verify: sudo systemctl status <name>
7. Check logs: journalctl -u <name> -f --no-pager -n 50

## Template
```ini
[Unit]
Description=<Service Description>
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=<user>
Group=<group>
WorkingDirectory=<working_dir>
ExecStart=<binary_path>
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

## Key Notes
- Always use Type=simple unless the process forks (then use Type=forking)
- Use RestartSec=5 to prevent restart storms
- Add security directives for production services
- Use journalctl -u <name> --since "5 min ago" for recent logs
"#;
