use clap::Parser as _;
use saurron::{
    cli::{Args, HeadWarnStrategy},
    config::{Config, DockerConfig},
    docker::{ContainerSelector, DockerClient},
    registry::{FreshnessResult, NonSemverStrategy, RegistryClient},
    update::UpdateEngine,
};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};

fn default_docker_config() -> DockerConfig {
    DockerConfig {
        host: "unix:///var/run/docker.sock".to_string(),
        tls_verify: false,
        tls_ca_cert: None,
        tls_cert: None,
        tls_key: None,
        api_version: None,
    }
}

/// Run a Docker CLI command and assert it succeeds.
fn docker_cmd(args: &[&str]) {
    let status = std::process::Command::new("docker")
        .args(args)
        .status()
        .expect("failed to invoke docker CLI");
    assert!(status.success(), "docker {:?} failed", args);
}

/// Start a `registry:2` container and return its ephemeral host port.
///
/// The returned `ContainerAsync` must stay alive for the duration of the test
/// (dropping it stops and removes the container).
async fn start_local_registry() -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new("registry", "2")
        .with_exposed_port(5000.tcp())
        .with_wait_for(WaitFor::message_on_stderr("listening on"))
        .start()
        .await
        .expect("failed to start local registry container");
    let port = container
        .get_host_port_ipv4(5000.tcp())
        .await
        .expect("failed to get registry host port");
    (container, port)
}

/// Tag `source_image` and push it to `registry_image`, returning the manifest
/// digest printed by `docker push` (format: `sha256:...`).
fn tag_and_push(source_image: &str, registry_image: &str) -> String {
    docker_cmd(&["tag", source_image, registry_image]);
    let out = std::process::Command::new("docker")
        .args(["push", registry_image])
        .output()
        .expect("failed to invoke docker push");
    assert!(
        out.status.success(),
        "docker push failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // `docker push` prints a line like: "stable: digest: sha256:abc size: 123"
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .find(|l| l.contains("digest: sha256:"))
        .and_then(|l| l.split("digest: ").nth(1))
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or_default()
        .to_string()
}

// ── Test 1: Docker connectivity ───────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn docker_client_connect_and_ping() {
    let docker = DockerClient::connect(&default_docker_config()).expect("connect failed");
    docker.ping().await.expect("daemon ping failed");
}

// ── Test 2: freshness — up to date ───────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn registry_freshness_up_to_date() {
    let (_registry, port) = start_local_registry().await;
    let image_ref = format!("localhost:{port}/testimage:stable");

    docker_cmd(&["pull", "busybox:latest"]);
    let digest = tag_and_push("busybox:latest", &image_ref);
    assert!(!digest.is_empty(), "no digest from push");

    let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None)
        .expect("failed to build registry client");
    let result = client
        .check_freshness(&image_ref, Some(&digest), false, NonSemverStrategy::Digest)
        .await;

    assert!(
        matches!(result, FreshnessResult::UpToDate),
        "expected UpToDate, got {result:?}"
    );
}

// ── Test 3: freshness — stale (digest changed for same tag) ──────────────────

#[tokio::test]
#[ignore]
async fn registry_freshness_stale_non_semver() {
    let (_registry, port) = start_local_registry().await;
    let image_ref = format!("localhost:{port}/testimage:stable");

    // Push busybox under the tag and record its digest.
    docker_cmd(&["pull", "busybox:latest"]);
    let old_digest = tag_and_push("busybox:latest", &image_ref);
    assert!(!old_digest.is_empty(), "no digest from first push");

    // Overwrite the tag in the registry with a different image (alpine).
    docker_cmd(&["pull", "alpine:latest"]);
    tag_and_push("alpine:latest", &image_ref);

    // check_freshness with the old (busybox) digest: registry now has alpine → Stale.
    let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None)
        .expect("failed to build registry client");
    let result = client
        .check_freshness(
            &image_ref,
            Some(&old_digest),
            false,
            NonSemverStrategy::Digest,
        )
        .await;

    assert!(
        matches!(result, FreshnessResult::Stale(_)),
        "expected Stale, got {result:?}"
    );
}

// ── Test 4: freshness — stale (higher SemVer tag available) ──────────────────

#[tokio::test]
#[ignore]
async fn registry_freshness_semver_stale() {
    let (_registry, port) = start_local_registry().await;
    let v100 = format!("localhost:{port}/myapp:v1.0.0");
    let v110 = format!("localhost:{port}/myapp:v1.1.0");

    docker_cmd(&["pull", "busybox:latest"]);
    tag_and_push("busybox:latest", &v100);
    tag_and_push("busybox:latest", &v110);

    let client = RegistryClient::new(HeadWarnStrategy::Auto, "test", None)
        .expect("failed to build registry client");
    let result = client
        .check_freshness(&v100, None, false, NonSemverStrategy::Digest)
        .await;

    match result {
        FreshnessResult::Stale(info) => {
            assert!(
                info.new_image.contains("v1.1.0"),
                "expected new_image to contain v1.1.0, got {}",
                info.new_image
            );
        }
        other => panic!("expected Stale, got {other:?}"),
    }
}

// ── Test 5: webhook notification ──────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn webhook_dispatch_posts_to_local_server() {
    use axum::{Router, body::Bytes, http::StatusCode, routing::post};
    use saurron::{
        config::WebhookConfig,
        notifications::{render_template, send_webhook},
        update::SessionReport,
    };
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let received: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let received_clone = Arc::clone(&received);

    let app = Router::new().route(
        "/hook",
        post(move |body: Bytes| {
            let slot = Arc::clone(&received_clone);
            async move {
                *slot.lock().await = Some(String::from_utf8_lossy(&body).into_owned());
                StatusCode::OK
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let report = SessionReport {
        updated: vec!["nginx".to_string()],
        ..Default::default()
    };
    let body = render_template(&report, None).unwrap();

    let cfg = WebhookConfig {
        url: format!("http://127.0.0.1:{port}/hook"),
        headers: Some("X-Saurron-Test: yes".to_string()),
        tls_skip_verify: false,
    };

    send_webhook(&cfg, &body)
        .await
        .expect("send_webhook failed");

    let got = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if let Some(v) = received.lock().await.take() {
                return v;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for webhook body");

    assert!(
        got.contains("nginx"),
        "body should mention updated container"
    );
    assert!(got.contains("Saurron update report"));
}

// ── Test 6: full update cycle ─────────────────────────────────────────────────

/// Ensures a container running an old SemVer image gets updated to the newer
/// version by `UpdateEngine::run_cycle`, exercising the complete
/// pull → stop → remove → create → start flow against a real Docker daemon.
#[tokio::test]
#[ignore]
async fn update_cycle_updates_stale_container() {
    const CONTAINER: &str = "saurron-integ-update";

    // CleanupGuard removes the test container even when the test panics.
    struct CleanupGuard(&'static str);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("docker")
                .args(["rm", "-f", self.0])
                .output();
        }
    }
    let _guard = CleanupGuard(CONTAINER);

    // 1. Local registry + two SemVer image versions.
    let (_registry, port) = start_local_registry().await;
    let v1 = format!("localhost:{port}/testapp:v1.0.0");
    let v2 = format!("localhost:{port}/testapp:v1.1.0");

    docker_cmd(&["pull", "busybox:latest"]);
    tag_and_push("busybox:latest", &v1);
    tag_and_push("busybox:latest", &v2);

    // 2. Start a container on the old version. Label enables opt-in selection.
    docker_cmd(&[
        "run",
        "-d",
        "--name",
        CONTAINER,
        "-l",
        "saurron.enable=true",
        &v1,
        "sleep",
        "3600",
    ]);

    // 3. Build engine components.
    let docker = DockerClient::connect(&default_docker_config()).expect("docker connect failed");
    let registry =
        RegistryClient::new(HeadWarnStrategy::Auto, "test", None).expect("registry client failed");
    let config = Config::load(&Args::parse_from(["saurron"])).expect("config load failed");

    // Opt-in selector: only containers with saurron.enable=true.
    let selector = ContainerSelector::new(true, false, &[], &[], false, false);
    let all = docker
        .list_containers(&selector)
        .await
        .expect("list_containers failed");
    let selected: Vec<_> = all.into_iter().filter(|c| c.name == CONTAINER).collect();

    assert!(
        !selected.is_empty(),
        "test container '{CONTAINER}' not found in running containers"
    );

    // 4. Run one update cycle.
    let report = UpdateEngine::new(&docker, &registry, &config)
        .run_cycle(&selected)
        .await;

    // 5. Assert the container was updated.
    assert!(
        report.updated.contains(&CONTAINER.to_string()),
        "expected '{CONTAINER}' in updated list, got: {:?}",
        report
    );
    assert!(
        report.failed.is_empty(),
        "unexpected failures: {:?}",
        report.failed
    );

    // 6. Verify the running container image is now v1.1.0.
    let inspect = docker
        .inspect_container(CONTAINER)
        .await
        .expect("inspect after update failed");
    let image = inspect
        .config
        .as_ref()
        .and_then(|c| c.image.as_deref())
        .unwrap_or("");
    assert!(
        image.contains("v1.1.0"),
        "expected container image to contain 'v1.1.0', got: '{image}'"
    );
}
