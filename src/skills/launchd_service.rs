pub const NAME: &str = "launchd_service";
pub const DESCRIPTION: &str = "Create and manage a launchd service (LaunchAgent or LaunchDaemon) on macOS";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. Determine if this should be a LaunchAgent (runs when user logs in) or LaunchDaemon (runs at system boot).
2. Create the property list (.plist) file using extract_and_write:
   - Agents: ~/Library/LaunchAgents/<label>.plist
   - Daemons: /Library/LaunchDaemons/<label>.plist (requires sudo)
3. Set correct permissions: chmod 644 <path>
4. Load the service: launchctl bootstrap <domain-target> <path>
   - User domain: gui/$(id -u)
   - System domain: system
5. Start if not automatic: launchctl kickstart -p <domain-target>/<label>
6. Verify: launchctl list | grep <label>
7. Check logs: tail -f /tmp/<label>.out.log (if configured in plist)

## Template
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.tempest.myservice</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/myscript</string>
        <string>arg1</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/com.tempest.myservice.out.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/com.tempest.myservice.err.log</string>
    <key>WorkingDirectory</key>
    <string>/Users/shared</string>
</dict>
</plist>
```

## Key Notes
- macOS uses launchctl for control.
- Use sudo launchctl only for LaunchDaemons.
- The Label must be unique (usually reverse-domain notation).
- launchctl bootstrap/bootout are the modern commands replacing load/unload.
- Use Console.app to view system-level logs if paths aren't specified.
"#;
