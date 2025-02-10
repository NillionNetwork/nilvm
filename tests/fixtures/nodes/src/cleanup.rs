use once_cell::sync::Lazy;
use std::{process::Child, sync::Mutex};
use xshell::{cmd, Shell};

static CHILD_PROCESSES: Lazy<Mutex<Vec<Process>>> = Lazy::new(Default::default);

enum Process {
    // A child process.
    Direct(Child),

    // A child process for which we know the parent that spawned it.
    Parent(Child),
}

pub(crate) fn register_parent_process(child: Child) {
    CHILD_PROCESSES.lock().unwrap().push(Process::Parent(child));
}

pub(crate) fn register_child_process(child: Child) {
    CHILD_PROCESSES.lock().unwrap().push(Process::Direct(child));
}

#[allow(clippy::significant_drop_in_scrutinee)]
pub(crate) fn kill_child_processes() {
    let mut children = CHILD_PROCESSES.lock().unwrap();

    eprintln!("Shutting down processes");
    for process in children.iter_mut() {
        match process {
            Process::Direct(child) => {
                let pid = child.id().to_string();
                eprintln!("Killing process with pid {pid}");
                if let Err(e) = child.kill() {
                    eprintln!("Error killing process: {e}");
                }
            }
            Process::Parent(child) => {
                let pid = child.id().to_string();
                eprintln!("Killing parent process with pid {pid}");

                // We are killing the process by PPID due to Just process is not propagating the KILL signal.
                let sh = Shell::new().unwrap();
                if let Err(e) = cmd!(sh, "pkill -KILL -P {pid}").run() {
                    eprintln!("Error killing process: {e}");
                }
            }
        };
    }
    for process in children.iter_mut() {
        let child = match process {
            Process::Direct(child) => child,
            Process::Parent(child) => child,
        };
        eprintln!("Waiting for node with pid {} to shutdown", child.id());
        let _ = child.wait();
    }
}
