use std::{
    ffi::{c_void, OsString},
    fs::OpenOptions,
    io::Write,
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_NAME: &str = "ElamPplTest";
const LOG_PATH: &str = r"C:\Windows\Temp\ppl_test.log";

define_windows_service!(ffi_service_main, service_main);

fn main() {
    if let Err(e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        eprintln!("Service dispatcher failed: {e}");
        eprintln!("This binary must be installed and started as a Windows service.");
        eprintln!("See README — Testing section.");
        std::process::exit(1);
    }
}

fn service_main(_args: Vec<OsString>) {
    let _ = run_service();
}

fn run_service() -> windows_service::Result<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let handler = move |control| match control {
        ServiceControl::Stop => {
            let _ = shutdown_tx.send(());
            ServiceControlHandlerResult::NoError
        },
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };

    let status = service_control_handler::register(SERVICE_NAME, handler)?;

    status.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    log(&format!("PPL test service started, PID={}", std::process::id()));
    log(&format!("Protection level: {}", protection_level_text()));

    loop {
        match shutdown_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(()) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => log("still alive"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    log("received stop, exiting");

    status.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

fn log(msg: &str) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

// PROCESS_INFORMATION_CLASS::ProcessProtectionLevelInfo
const PROCESS_PROTECTION_LEVEL_INFO: u32 = 7;

#[repr(C)]
struct ProcessProtectionLevelInformation {
    protection_level: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetProcessInformation(
        process: *mut c_void,
        information_class: u32,
        information: *mut c_void,
        information_size: u32,
    ) -> i32;
}

fn protection_level_text() -> String {
    let mut info = ProcessProtectionLevelInformation { protection_level: 0 };
    let ok = unsafe {
        GetProcessInformation(
            GetCurrentProcess(),
            PROCESS_PROTECTION_LEVEL_INFO,
            &mut info as *mut _ as *mut c_void,
            core::mem::size_of::<ProcessProtectionLevelInformation>() as u32,
        )
    };
    if ok == 0 {
        return "<query failed>".into();
    }
    let level = info.protection_level;
    let name = match level {
        0x00000000 => "WinTcb-Light",
        0x00000001 => "Windows",
        0x00000002 => "Windows-Light",
        0x00000003 => "Antimalware-Light",
        0x00000004 => "Lsa-Light",
        0x00000005 => "WinTcb",
        0x00000006 => "CodeGen-Light",
        0x00000007 => "Authenticode",
        0x00000008 => "PPL-App",
        0xFFFFFFFE => "None (not protected)",
        _ => "Unknown",
    };
    format!("0x{level:08x} ({name})")
}
