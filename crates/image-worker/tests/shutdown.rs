use std::{
    env,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use pp_database::{Database, PgPool, tests::TestRepository};
use url::Url;

/// Runs the real binary against a scratch database: it must apply the migrations, idle on the
/// empty queue and exit cleanly on SIGTERM.
#[sqlx::test]
async fn migrates_and_exits_cleanly_on_sigterm(pool: PgPool) {
    let photos = tempfile::tempdir().expect("photo directory should be created");
    let mut url = Url::parse(&env::var("DATABASE_URL").expect("DATABASE_URL is required"))
        .expect("DATABASE_URL should be a URL");
    url.set_path(
        pool.connect_options()
            .get_database()
            .expect("test database name"),
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_pp-image-worker"))
        .env("PP_DATABASE_URL", url.as_str())
        .env("PP_PHOTO_DIR", photos.path())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker binary should spawn");

    let database = Database::new(pool.clone());
    let start = Instant::now();
    loop {
        let migrated = database
            .repository::<TestRepository>()
            .is_migrated()
            .await
            .expect("jobs lookup should work");
        if migrated {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "worker did not migrate"
        );
        thread::sleep(Duration::from_millis(50));
    }
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
        thread::sleep(Duration::from_millis(50));
    }
    child.kill().expect("hung worker should be killable");
    child.wait().expect("killed worker should be reaped");
    panic!("worker did not exit after SIGTERM");
}

#[test]
fn refuses_to_start_without_a_photo_directory() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let output = Command::new(env!("CARGO_BIN_EXE_pp-image-worker"))
        .env("PP_PHOTO_DIR", dir.path().join("missing"))
        .output()
        .expect("worker binary should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("photo directory"));
}
