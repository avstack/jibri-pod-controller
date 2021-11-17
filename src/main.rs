#![forbid(unsafe_code)]

mod jibri_schema;
mod server;
mod sweeper;

use std::{
  env,
  sync::{
    atomic::{self, AtomicBool},
    Arc,
  },
  time::Duration,
};

use anyhow::Context;
use hyper::Client;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use kube_leader_election::{LeaseLock, LeaseLockParams};
use once_cell::sync::Lazy;
use tokio::{
  signal::unix::{signal, SignalKind},
  sync::{oneshot, watch},
  time::{interval, MissedTickBehavior},
};
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::{server::server, sweeper::sweeper};

static SWEEP_INTERVAL: Lazy<Duration> = Lazy::new(|| {
  Duration::from_secs(
    env::var("SWEEP_INTERVAL")
      .expect("missing SWEEP_INTERVAL env var")
      .parse()
      .expect("invalid SWEEP_INTERVAL env var"),
  )
});

static PORT: Lazy<u16> = Lazy::new(|| {
  env::var("PORT")
    .expect("missing PORT env var")
    .parse()
    .expect("invalid PORT env var")
});

static JIBRI_HEALTH_PORT: Lazy<u16> = Lazy::new(|| {
  env::var("JIBRI_HEALTH_PORT")
    .expect("missing JIBRI_HEALTH_PORT env var")
    .parse()
    .expect("invalid JIBRI_HEALTH_PORT env var")
});

static JIBRI_BUSY_LABELS: Lazy<String> =
  Lazy::new(|| env::var("JIBRI_BUSY_LABELS").expect("missing JIBRI_BUSY_LABELS env var"));

static POD_NAME: Lazy<String> =
  Lazy::new(|| env::var("POD_NAME").expect("missing POD_NAME env var"));

static NAMESPACE: Lazy<String> =
  Lazy::new(|| env::var("NAMESPACE").expect("missing NAMESPACE env var"));

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .with_span_events(FmtSpan::CLOSE)
    .with_target(false)
    .without_time()
    .init();

  let shutdown = Arc::new(AtomicBool::new(false));
  let (graceful_shutdown_tx, graceful_shutdown_rx) = oneshot::channel();
  {
    let shutdown = shutdown.clone();
    tokio::spawn(async move {
      let mut sigterm = signal(SignalKind::terminate()).unwrap();
      sigterm.recv().await;
      shutdown.store(true, atomic::Ordering::Relaxed);
      let _ = graceful_shutdown_tx.send(());
    });
  }

  let k8s_client = kube::Client::try_default()
    .await
    .expect("k8s client setup failed");

  let leadership = Arc::new(LeaseLock::new(
    k8s_client.clone(),
    &*NAMESPACE,
    LeaseLockParams {
      holder_id: POD_NAME.clone(),
      lease_name: "jibri-pod-controller-lease".into(),
      lease_ttl: *SWEEP_INTERVAL * 2,
    },
  ));

  let (leaser, sweeper_lease_rx) = {
    let leadership = leadership.clone();
    let shutdown = shutdown.clone();
    let (tx, rx) = watch::channel(false);
    (
      tokio::spawn(async move {
        let mut interval = interval(*SWEEP_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
          let lease = leadership
            .try_acquire_or_renew()
            .await
            .context("failed to acquire or renew leadership lease")?;
          tx.send(lease.acquired_lease)?;

          if shutdown.load(atomic::Ordering::Relaxed) {
            info!("exiting lease task");
            break;
          }

          interval.tick().await;
        }
        Ok::<_, anyhow::Error>(())
      }),
      rx,
    )
  };

  let http_client = Client::new();

  let pods: Api<Pod> = Api::namespaced(k8s_client, "default");

  let server = tokio::spawn(server(*PORT, pods.clone(), graceful_shutdown_rx));
  let sweeper = tokio::spawn(sweeper(http_client, pods, sweeper_lease_rx, shutdown));

  tokio::select!(
    res = leaser => res.unwrap().unwrap(),
    res = server => res.unwrap().unwrap(),
    res = sweeper => res.unwrap().unwrap(),
  );
}
