use std::{collections::BTreeMap, convert::Infallible, net::SocketAddr};

use anyhow::Result;
use futures::future::FutureExt;
use hyper::{
  body::Buf,
  service::{make_service_fn, service_fn},
  Body, Method, Request, Response, Server, StatusCode,
};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, DeleteParams, Patch, PatchParams};
use serde_json::json;
use tokio::sync::oneshot;
use tracing::info;

use crate::{
  jibri_schema::{JibriBusyStatus, JibriWebhookRequest},
  JIBRI_BUSY_LABELS,
};

pub async fn server(
  port: u16,
  pods: Api<Pod>,
  graceful_shutdown_rx: oneshot::Receiver<()>,
) -> Result<()> {
  let bind = SocketAddr::from(([0, 0, 0, 0], port));
  info!(%bind);

  let make_service = {
    let pods = pods.clone();
    make_service_fn(move |_sock| {
      let pods = pods.clone();
      async move {
        Ok::<_, Infallible>(service_fn(move |req| {
          let pods = pods.clone();
          async move {
            if req.uri().path() == "/" {
              return match *req.method() {
                Method::GET | Method::HEAD | Method::OPTIONS => Ok(
                  Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::empty())?,
                ),
                _ => method_not_allowed(&["GET", "HEAD", "OPTIONS"]),
              };
            }

            match handle_request(req, pods).await {
              Ok(res) => Ok::<_, anyhow::Error>(res),
              Err(_) => internal_server_error(),
            }
          }
        }))
      }
    })
  };

  let server = Server::bind(&bind)
    .http1_keepalive(true)
    .http1_only(true)
    .tcp_nodelay(true)
    .serve(make_service)
    .with_graceful_shutdown(graceful_shutdown_rx.map(|_| ()));

  Ok(server.await?)
}

#[tracing::instrument(level = "debug", skip(pods), err)]
async fn handle_request(req: Request<Body>, pods: Api<Pod>) -> Result<Response<Body>> {
  let mut path_parts = req.uri().path().split('/');

  if path_parts.next() != Some("") {
    return bad_request();
  }

  let path: Vec<_> = path_parts.collect();

  match &path[..] {
    ["webhook", pod_name, "v1", "status"] => match *req.method() {
      Method::POST => {
        let pod_name = pod_name.to_string();
        let webhook_body: JibriWebhookRequest =
          serde_json::from_reader(hyper::body::aggregate(req.into_body()).await?.reader())?;

        info!(%pod_name, ?webhook_body);

        if webhook_body.status.busy_status == JibriBusyStatus::Expired {
          pods.delete(&pod_name, &DeleteParams::default()).await?;
        }
        else if webhook_body.status.busy_status == JibriBusyStatus::Busy {
          let labels: BTreeMap<String, String> = JIBRI_BUSY_LABELS
            .split(',')
            .map(|label| {
              let mut parts = label.splitn(2, '=');
              (
                parts.next().unwrap().to_owned(),
                parts.next().unwrap_or_default().to_owned(),
              )
            })
            .collect();

          let patch = json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
              "labels": labels,
              "annotations": {"cluster-autoscaler.kubernetes.io/safe-to-evict": "false"},
            },
          });
          pods
            .patch(
              &pod_name,
              &PatchParams::apply("jibri-pod-controller").force(),
              &Patch::Apply(&patch),
            )
            .await?;
        }

        Ok(
          Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())?,
        )
      },
      Method::OPTIONS => options(&["POST", "OPTIONS"]),
      _ => method_not_allowed(&["POST", "OPTIONS"]),
    },
    _ => not_found(),
  }
}

#[tracing::instrument(level = "info")]
fn not_found() -> Result<Response<Body>> {
  Ok(
    Response::builder()
      .status(StatusCode::NOT_FOUND)
      .body(Body::empty())?,
  )
}

#[tracing::instrument(level = "info")]
fn method_not_allowed(allowed_methods: &[&'static str]) -> Result<Response<Body>> {
  Ok(
    Response::builder()
      .status(StatusCode::METHOD_NOT_ALLOWED)
      .header("allow", allowed_methods.join(", "))
      .body(Body::empty())?,
  )
}

#[tracing::instrument(level = "info")]
fn bad_request() -> Result<Response<Body>> {
  Ok(
    Response::builder()
      .status(StatusCode::BAD_REQUEST)
      .body(Body::empty())?,
  )
}

#[tracing::instrument(level = "info")]
fn forbidden() -> Result<Response<Body>> {
  Ok(
    Response::builder()
      .status(StatusCode::FORBIDDEN)
      .body(Body::empty())?,
  )
}

#[tracing::instrument(level = "info")]
fn internal_server_error() -> Result<Response<Body>> {
  Ok(
    Response::builder()
      .status(StatusCode::INTERNAL_SERVER_ERROR)
      .body(Body::empty())?,
  )
}

#[tracing::instrument(level = "info")]
fn options(allowed_methods: &[&'static str]) -> Result<Response<Body>> {
  Ok(
    Response::builder()
      .status(StatusCode::NO_CONTENT)
      .header("allow", allowed_methods.join(", "))
      .body(Body::empty())?,
  )
}
