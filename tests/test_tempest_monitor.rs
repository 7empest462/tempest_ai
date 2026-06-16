#[test]
fn test_inspect_monitor() {
    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let pid = std::process::id();
    assert!(sys.processes().contains_key(&sysinfo::Pid::from(pid as usize)));
}
