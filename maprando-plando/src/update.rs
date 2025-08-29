use anyhow::{anyhow, bail, Result};
use serde_json::Value;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct Asset {
    pub url: String,
    pub name: String,
}

#[derive(Clone)]
pub struct Release {
    pub tag_name: String,
    pub version: String,
    pub assets: Vec<Asset>
}

impl Release {
    fn from_value(v: &Value) -> Option<Self> {
        let tag_name = v.get("tag_name")?.as_str()?.to_string();
        let version = tag_name.trim_start_matches('v').to_string();
        let asset_values = v.get("assets")?.as_array();
        let asset_len = asset_values.map(|x| x.len()).unwrap_or(0);
        let mut assets = Vec::with_capacity(asset_len);

        if let Some(vec) = asset_values {
            for asset in vec {
                let url = asset.get("url")?.as_str()?.to_string();
                let name = asset.get("name")?.as_str()?.to_string();
                assets.push(Asset {
                    url, name,
                });
            }
        }

        Some(Release {
            tag_name, version, assets
        })
    }
}

pub async fn check_update() -> Result<Release> {
    let api_url = "https://api.github.com/repos/noktuska/maprando-plando/releases".to_string();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, "reqwest/maprando-plando".parse()?);
    let client = reqwest::ClientBuilder::new().use_rustls_tls().http2_adaptive_window(true).build()?;
    let resp = client.get(&api_url).headers(headers).query( &[("per_page", "100")]).send().await?;
    if !resp.status().is_success() {
        bail!("API request failed with status: {:?} - for: {}", resp.status(), api_url);
    }

    let releases = resp.json::<serde_json::Value>().await?;
    let releases = releases.as_array().ok_or_else(|| anyhow!("No releases found"))?;
    let mut releases = releases.into_iter().filter_map(|elem| {
        Release::from_value(elem)
    }).collect::<Vec<_>>();
    if releases.is_empty() {
        bail!("No releases found");
    }
    let latest_release = releases.swap_remove(0);

    if !is_version_higher(&latest_release.version, "0.1.3") {
        bail!("Current version is up to date");
    }

    Ok(latest_release)
}

fn is_version_higher(v1: &str, v2: &str) -> bool {
    let v1_splits = v1.split('.').collect::<Vec<_>>();
    let v2_splits = v2.split('.').collect::<Vec<_>>();

    let common_len = v1_splits.len().min(v2_splits.len());
    for i in 0..common_len {
        let elem1: usize = v1_splits[i].parse().unwrap();
        let elem2: usize = v2_splits[i].parse().unwrap();

        if elem1 > elem2 {
            return true;
        } else if elem2 > elem1 {
            return false;
        }
    }

    v1_splits.len() > v2_splits.len()
}