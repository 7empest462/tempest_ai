pub const NAME: &str = "cron_scheduler";
pub const DESCRIPTION: &str = "Set up scheduled tasks with cron or systemd timers";
pub const INSTRUCTIONS: &str = r#"
## Cron
1. Edit crontab: crontab -e
2. Format: MIN HOUR DOM MON DOW command
   - */5 * * * * — every 5 minutes
   - 0 */2 * * * — every 2 hours
   - 0 3 * * * — daily at 3 AM
   - 0 0 * * 0 — weekly on Sunday midnight
   - 0 0 1 * * — monthly on the 1st
3. Always redirect output: command >> /var/log/myjob.log 2>&1
4. Verify: crontab -l

## Systemd Timers (Modern Alternative)
Create two files:

### /etc/systemd/system/myjob.service
```ini
[Unit]
Description=My Scheduled Job

[Service]
Type=oneshot
ExecStart=/path/to/script.sh
User=<user>
```

### /etc/systemd/system/myjob.timer
```ini
[Unit]
Description=Run My Job on schedule

[Timer]
OnCalendar=*-*-* 03:00:00
Persistent=true

[Install]
WantedBy=timers.target
```

Then: sudo systemctl enable --now myjob.timer
Check: systemctl list-timers --all

## Key Notes
- Systemd timers are preferred over cron on modern Linux
- Persistent=true means if the system was off during the scheduled time, it runs on next boot
- Cron doesn't load your shell profile — use full paths to commands
- Test cron jobs manually before scheduling
"#;
