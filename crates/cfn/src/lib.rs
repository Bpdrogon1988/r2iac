
use anyhow::{Context, Result};
use serde_json::Value as Json;
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum CfnRunner { AwsCli }

fn aws() -> Result<String> {
    let p = which::which("aws").context("aws cli not found in PATH")?;
    Ok(p.to_string_lossy().into_owned())
}

pub fn deploy_stack(stack_name: &str, template_body: &Json, region: Option<&str>) -> Result<()> {
    let aws = aws()?;
    let mut cmd = Command::new(aws);
    cmd.arg("cloudformation").arg("deploy")
        .arg("--stack-name").arg(stack_name)
        .arg("--template-file").arg("-")
        .arg("--capabilities").arg("CAPABILITY_NAMED_IAM");
    if let Some(r) = region { cmd.arg("--region").arg(r); }
    let mut child = cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn().context("spawn aws cloudformation deploy")?;
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        let s = serde_json::to_string_pretty(template_body)?;
        stdin.write_all(s.as_bytes())?;
    }
    let st = child.wait()?;
    if !st.success() { anyhow::bail!("cloudformation deploy failed") }
    Ok(())
}

pub fn delete_stack(stack_name: &str, region: Option<&str>) -> Result<()> {
    let aws = aws()?;
    let mut cmd = Command::new(aws);
    cmd.arg("cloudformation").arg("delete-stack")
        .arg("--stack-name").arg(stack_name);
    if let Some(r) = region { cmd.arg("--region").arg(r); }
    let st = cmd.status().context("aws cloudformation delete-stack")?;
    if !st.success() { anyhow::bail!("cloudformation delete-stack failed") }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CfnAnyResource {
    pub name: String,
    #[serde(rename="Type")]
    pub type_name: String,
    #[serde(default)]
    pub properties: serde_json::Map<String, Json>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CfnTemplate {
    #[serde(rename="AWSTemplateFormatVersion")] pub version: Option<String>,
    #[serde(rename="Description")] pub description: Option<String>,
    #[serde(rename="Resources")] pub resources: std::collections::BTreeMap<String, CfnAnyResource>,
}

