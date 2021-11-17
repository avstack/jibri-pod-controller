use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JibriWebhookRequest {
  pub jibri_id: String,
  pub status: JibriStatus,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JibriStatus {
  pub busy_status: JibriBusyStatus,
  pub health: JibriHealth,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JibriBusyStatus {
  Idle,
  Busy,
  Expired,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JibriHealth {
  pub health_status: JibriHealthStatus,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JibriHealthStatus {
  Healthy,
  Unhealthy,
}
