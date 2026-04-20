use reqwest::Client;
use serde_json::json;

pub struct SyncthingClient {
    client: Client,
    api_url: String,
    api_key: Option<String>,
}

impl SyncthingClient {
    pub fn new(api_url: &str, api_key: Option<&str>) -> Self {
        Self {
            client: Client::new(),
            api_url: api_url.trim_end_matches('/').to_string(),
            api_key: api_key.map(|s| s.to_string()),
        }
    }

    fn build_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.api_url, path);
        let mut req = self.client.request(method, &url);
        if let Some(key) = &self.api_key {
            req = req.header("X-API-Key", key);
        }
        req
    }

    /// 创建或更新一个 folder。
    /// 参考 syncthing 探索结果：POST /rest/config/folders 最小需要 id + path。
    /// device_ids 若为空数组，Syncthing 会自动把本机设备加入。
    pub async fn create_or_update_folder(
        &self,
        id: &str,
        path: &str,
        device_ids: &[String],
    ) -> anyhow::Result<()> {
        let devices: Vec<serde_json::Value> =
            device_ids.iter().map(|d| json!({ "deviceID": d })).collect();
        let body = json!({
            "id": id,
            "path": path,
            "devices": devices,
        });
        let resp = self
            .build_request(reqwest::Method::POST, "/rest/config/folders")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Syncthing API error: {}", text);
        }
        Ok(())
    }

    /// 查询 folder 状态：GET /rest/db/status?folder=<id>
    pub async fn get_folder_status(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        let resp = self
            .build_request(reqwest::Method::GET, &format!("/rest/db/status?folder={}", id))
            .send()
            .await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Syncthing API error: {}", text);
        }
        Ok(resp.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new_with_trailing_slash() {
        let client = SyncthingClient::new("http://localhost:8384/", Some("key123"));
        let _ = client;
    }

    #[test]
    fn test_client_new_without_trailing_slash() {
        let client = SyncthingClient::new("http://localhost:8384", None);
        let _ = client;
    }
}
