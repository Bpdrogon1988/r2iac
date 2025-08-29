use serde::{Serialize, Deserialize};
use serde_json::{json, Value as Json, Map as JsonMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpProvider { pub project: String, pub region: Option<String> }
impl GcpProvider {
    pub fn to_tf_json(&self) -> Json {
        let mut body = json!({ "project": self.project });
        if let Some(r) = &self.region { body["region"] = json!(r); }
        json!({ "provider": { "google": body } })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag="type")]
pub enum GcpResource {
    #[serde(rename="google_storage_bucket")]
    StorageBucket { name: String, location: String, force_destroy: Option<bool> },
    #[serde(rename="google_kms_key_ring")]
    KmsKeyRing { name: String, location: String },
    #[serde(rename="google_secret_manager_secret")]
    SecretManagerSecret { name: String, replication: String },
}

impl GcpResource {
    pub fn to_tf_json(&self) -> Json {
        match self {
            GcpResource::StorageBucket { name, location, force_destroy } => {
                let mut body = json!({ "name": name, "location": location });
                if let Some(f) = force_destroy { body["force_destroy"] = json!(f); }
                json!({ "resource": { "google_storage_bucket": { name: body } } })
            }
            GcpResource::KmsKeyRing { name, location } => json!({
                "resource": { "google_kms_key_ring": { name: { "name": name, "location": location } } }
            }),
            GcpResource::SecretManagerSecret { name, replication } => json!({
                "resource": { "google_secret_manager_secret": { name: { "name": name, "replication": replication } } }
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpAnyResource {
    #[serde(rename="type")]
    pub type_name: String,
    pub name: String,
    #[serde(flatten)]
    pub properties: JsonMap<String, Json>,
}

impl GcpAnyResource {
    pub fn to_tf_json(&self) -> Json {
        let mut props = self.properties.clone();
        if !props.contains_key("name") {
            props.insert("name".to_string(), json!(self.name.clone()));
        }
        json!({
            "resource": {
                self.type_name.clone(): {
                    self.name.clone(): props
                }
            }
        })
    }
}

