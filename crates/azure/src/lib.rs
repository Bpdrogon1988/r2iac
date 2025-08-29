use serde::{Serialize, Deserialize};
use serde_json::{json, Value as Json, Map as JsonMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureProvider { #[serde(default)] pub features: JsonMap<String, Json>, pub subscription_id: Option<String> }
impl AzureProvider {
    pub fn to_tf_json(&self) -> Json {
        let mut provider = json!({ "features": self.features });
        if let Some(sid) = &self.subscription_id { provider["subscription_id"] = json!(sid); }
        json!({ "provider": { "azurerm": provider } })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureAnyResource {
    #[serde(rename="type")]
    pub type_name: String,
    pub name: String,
    #[serde(flatten)]
    pub properties: JsonMap<String, Json>,
}

impl AzureAnyResource {
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

