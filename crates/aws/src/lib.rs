use serde::{Serialize, Deserialize};
use serde_json::{json, Value as Json, Map as JsonMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsProvider { pub region: String }
impl AwsProvider {
    pub fn to_tf_json(&self) -> Json {
        json!({ "provider": { "aws": { "region": self.region } } })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag="type")]
pub enum AwsResource {
    #[serde(rename="aws_s3_bucket")]
    S3Bucket {
        name: String,
        bucket: String,
        #[serde(default)]
        force_destroy: bool,
        #[serde(default)]
        kms_key_id: Option<String>,
    },
    #[serde(rename="aws_kms_key")]
    KmsKey {
        name: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        enable_key_rotation: bool,
        #[serde(default)]
        deletion_window_in_days: Option<u32>,
        #[serde(default)]
        key_usage: Option<String>,
        #[serde(default)]
        key_spec: Option<String>,
    },
    #[serde(rename="aws_secretsmanager_secret")]
    SecretsManagerSecret {
        name: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        kms_key_id: Option<String>,
        #[serde(default)]
        recovery_window_in_days: Option<u32>,
        #[serde(default)]
        force_delete_without_recovery: Option<bool>,
    },
}

impl AwsResource {
    pub fn to_tf_json(&self) -> Json {
        match self {
            AwsResource::S3Bucket { name, bucket, force_destroy, kms_key_id } => {
                let mut o = json!({
                  "resource": { "aws_s3_bucket": {
                      name: { "bucket": bucket, "force_destroy": force_destroy }
                  }}
                });
                o["resource"]["aws_s3_bucket"][name]["bucket_encryption"] =
                  if let Some(kms) = kms_key_id {
                    json!({ "server_side_encryption_configuration": {
                      "rule": { "apply_server_side_encryption_by_default": {
                        "sse_algorithm":"aws:kms", "kms_master_key_id": kms
                      }}}})
                  } else {
                    json!({ "server_side_encryption_configuration": {
                      "rule": { "apply_server_side_encryption_by_default": {
                        "sse_algorithm":"AES256"
                      }}}})
                  };
                o
            }
            AwsResource::KmsKey { name, description, enable_key_rotation, deletion_window_in_days, key_usage, key_spec } => {
                let mut body = json!({
                    "enable_key_rotation": enable_key_rotation,
                });
                if let Some(desc) = description { body["description"] = json!(desc); }
                if let Some(days) = deletion_window_in_days { body["deletion_window_in_days"] = json!(days); }
                if let Some(u) = key_usage { body["key_usage"] = json!(u); }
                if let Some(s) = key_spec { body["key_spec"] = json!(s); }
                json!({
                    "resource": { "aws_kms_key": {
                        name: body
                    }}
                })
            }
            AwsResource::SecretsManagerSecret { name, description, kms_key_id, recovery_window_in_days, force_delete_without_recovery } => {
                let mut body = json!({});
                if let Some(desc) = description { body["description"] = json!(desc); }
                if let Some(kms) = kms_key_id { body["kms_key_id"] = json!(kms); }
                if let Some(days) = recovery_window_in_days { body["recovery_window_in_days"] = json!(days); }
                if let Some(force) = force_delete_without_recovery { body["force_delete_without_recovery"] = json!(force); }
                json!({
                    "resource": { "aws_secretsmanager_secret": {
                        name: body
                    }}
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsAnyResource {
    #[serde(rename="type")]
    pub type_name: String,
    pub name: String,
    #[serde(flatten)]
    pub properties: JsonMap<String, Json>,
}

impl AwsAnyResource {
    pub fn to_tf_json(&self) -> Json {
        let mut props = self.properties.clone();
        if !props.contains_key("bucket") && !props.contains_key("name") {
            // add a best-effort name/bucket to reduce typos causing missing identifier
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
