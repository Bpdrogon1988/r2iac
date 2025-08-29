

use anyhow::Result;
use serde_json::Value as Json;

/// Simple plan-time checks (expand later).
pub struct Policy { pub allow_unencrypted: bool }

impl Policy {
    pub fn new(allow_unencrypted: bool) -> Self { Self { allow_unencrypted } }

    pub fn check_tf_json(&self, tf: &Json) -> Result<()> {
        if let Some(res) = tf.get("resource").and_then(|r| r.get("aws_s3_bucket")) {
            for (_name, bucket) in res.as_object().unwrap().iter() {
                let has_enc = bucket.get("bucket_encryption").is_some()
                    || bucket.get("server_side_encryption_configuration").is_some();
                if !has_enc && !self.allow_unencrypted {
                    anyhow::bail!("Policy: S3 bucket requires encryption (SSE-S3 or KMS).");
                }
            }
        }
        Ok(())
    }
}
