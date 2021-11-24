use std::sync::{
  atomic::{self, AtomicBool},
  Arc,
};

use anyhow::Result;
use hyper::{body::Buf, client::HttpConnector, Client};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, DeleteParams, ListParams};
use tokio::{
  sync::watch,
  time::{interval, MissedTickBehavior},
};
use tracing::{debug, info};

use crate::{
  jibri_schema::{JibriBusyStatus, JibriStatus},
  JIBRI_BUSY_LABELS, JIBRI_HEALTH_PORT, SWEEP_INTERVAL,
};

pub async fn sweeper(
  http_client: Client<HttpConnector>,
  pods: Api<Pod>,
  sweeper_lease_rx: watch::Receiver<bool>,
  shutdown: Arc<AtomicBool>,
) -> Result<()> {
  let mut interval = interval(*SWEEP_INTERVAL);
  interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
  info!("In Sweeper");
  loop {
    info!("In Running sweeper Loop");
    if *sweeper_lease_rx.borrow() {
      let jibri_pods = pods
        .list(&ListParams::default().labels(&*JIBRI_BUSY_LABELS))
        .await?;
      info!("Running sweeper");
      for pod in jibri_pods {
        if let Some(name) = &pod.metadata.name {
          if let Some(ip) = pod
            .status
            .as_ref()
            .and_then(|status| status.pod_ip.as_ref())
          {
            info!("Health check API call for {}", ip);
            let uri = format!("http://{}:{}/jibri/api/v1.0/health", ip, *JIBRI_HEALTH_PORT);
            let res = http_client.get(uri.parse()?).await?;
            let status: JibriStatus =
              serde_json::from_reader(hyper::body::aggregate(res.into_body()).await?.reader())?;

            info!(%name, ?status);

            if status.busy_status == JibriBusyStatus::Expired {
              pods.delete(name, &DeleteParams::default()).await?;
              info!("Pod {} deleted successfully by jibri-pod-controller", name);
            }
          }
        }
      }
    }
    else {
      debug!("not leader, skipping sweep");
    }

    if shutdown.load(atomic::Ordering::Relaxed) {
      info!("shutdown complete");
      break;
    }

    interval.tick().await;
  }

  Ok(())
}
