#![allow(non_upper_case_globals)]

use core_foundation::runloop::{
    kCFRunLoopCommonModes, CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRun, __CFRunLoopSource,
};
use std::{ffi::c_void, sync::mpsc::{channel, Receiver, Sender}};
use IOKit_sys::{
    io_object_t, io_service_t, kIOMessageSystemHasPoweredOn, kIOMessageSystemWillPowerOn,
    kIOMessageSystemWillSleep, IONotificationPortGetRunLoopSource, IONotificationPortRef,
    IORegisterForSystemPower,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Sleep,
    Wake,
}

extern "C" fn power_callback(
    refcon: *mut c_void,
    _service: io_service_t,
    message_type: u32,
    _message_argument: *mut c_void,
) {
    let tx = unsafe { &*(refcon as *mut Sender<Activity>) };
    log::debug!("Power callback called: {:X?}", message_type);
    match message_type {
        kIOMessageSystemWillSleep => tx.send(Activity::Sleep).unwrap(),
        kIOMessageSystemWillPowerOn => tx.send(Activity::Wake).unwrap(),
        kIOMessageSystemHasPoweredOn => tx.send(Activity::Wake).unwrap(),
        _ => log::info!("Unknown message type: {:X?}", message_type),
    }
}

pub fn start_power_monitor() -> Receiver<Activity> {
    let (tx, rx) = channel::<Activity>();
    std::thread::spawn(move || unsafe {
        let tx_ptr = Box::into_raw(Box::new(tx));
        let mut notifier_port: IONotificationPortRef = std::ptr::null_mut();
        let mut notifier_object: io_object_t = 0;

        let root_port = IORegisterForSystemPower(
            tx_ptr as *mut c_void,
            &mut notifier_port,
            power_callback,
            &mut notifier_object,
        );
        if root_port == 0 {
            log::error!("Failed to register for system power notifications");
            return;
        }
        let run_loop_source = IONotificationPortGetRunLoopSource(notifier_port);
        CFRunLoopAddSource(
            CFRunLoopGetCurrent(),
            run_loop_source as *mut __CFRunLoopSource,
            kCFRunLoopCommonModes,
        );
        CFRunLoopRun();
    });
    rx
}
