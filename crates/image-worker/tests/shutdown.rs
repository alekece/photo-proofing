use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn exits_cleanly_on_sigterm() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_eg-image-worker"))
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker binary should spawn");
    std::thread::sleep(Duration::from_millis(700));
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(status) = child.try_wait().expect("child status should be readable") {
            let out = child
                .wait_with_output()
                .expect("exited child output should be readable");
            let text = String::from_utf8_lossy(&out.stdout).to_string()
                + &String::from_utf8_lossy(&out.stderr);
            assert!(status.success(), "exit status {status}");
            assert!(text.contains("image worker started"), "{text}");
            assert!(text.contains("image worker stopping"), "{text}");
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.kill().expect("hung worker should be killable");
    child.wait().expect("killed worker should be reaped");
    panic!("worker did not exit after SIGTERM");
}
