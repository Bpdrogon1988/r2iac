use anyhow::{Result, Context};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use tracing_subscriber::fmt::format::FmtSpan;
use secrecy::ExposeSecret;
use std::process::{Command, Stdio};

use r2iac_policy::Policy;
use r2iac_tfcompat as tfc;
use r2iac_aws::{AwsProvider, AwsResource, AwsAnyResource};
use r2iac_azure::{AzureProvider, AzureAnyResource};
use r2iac_gcp::{GcpProvider, GcpResource, GcpAnyResource};
use r2iac_cfn as cfn;

#[derive(Parser, Debug)]
#[command(author, version, about="r2iac — Rust IaC CLI (Terraform/OpenTofu compat)")]
struct Cli {
    /// Config file (YAML or .yml.age)
    #[arg(short, long, global = true)]
    file: PathBuf,

    /// Output directory
    #[arg(short, long, default_value="out", global = true)]
    out: PathBuf,

    /// Runner
    #[arg(long, value_enum, default_value_t=Runner::Auto, global = true)]
    runner: Runner,

    /// Allow unencrypted buckets
    #[arg(long, default_value_t=false, global = true)]
    allow_unencrypted: bool,

    /// AGE identities (optional, for .age files)
    #[arg(long="age-identity", global = true)]
    age_ids: Vec<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, ValueEnum)]
enum Runner { Auto, Terraform, Tofu }

#[derive(Subcommand, Debug)] enum Cmd {
    Init,
    Plan,
    Apply,
    Destroy,
    AwsConfigure {
        #[arg(long)] profile: Option<String>,
        #[arg(long)] access_key_id: Option<String>,
        #[arg(long)] secret_access_key: Option<String>,
        #[arg(long)] region: Option<String>,
    },
    CfnDeploy {
        #[arg(long)] stack: Option<String>,
        #[arg(short='f', long="file")] file: Option<PathBuf>,
        #[arg(short='o', long="out")] out: Option<PathBuf>,
    },
    CfnDelete {
        #[arg(long)] stack: Option<String>,
        #[arg(short='f', long="file")] file: Option<PathBuf>,
        #[arg(short='o', long="out")] out: Option<PathBuf>,
    }
}

#[derive(Deserialize)]
struct Stack { project: Option<String>, provider: Providers, resources: Vec<Resource> }
#[derive(Deserialize)] struct Providers { 
    #[serde(default)] aws: Option<AwsProvider>,
    #[serde(default)] azurerm: Option<AzureProvider>,
    #[serde(default)] google: Option<GcpProvider>,
}
#[derive(Deserialize, Clone)]
#[serde(tag="cloud")]
enum Resource { 
    #[serde(rename="aws")]   Aws   { #[serde(flatten)] res: AwsResource },
    #[serde(rename="aws_any")] AwsAny { #[serde(flatten)] res: AwsAnyResource },
    #[serde(rename="azure")] Azure { #[serde(flatten)] res: AzureAnyResource },
    #[serde(rename="gcp")]   Gcp   { #[serde(flatten)] res: GcpResource },
    #[serde(rename="gcp_any")] GcpAny { #[serde(flatten)] res: GcpAnyResource },
}

fn merge(mut a: Json, b: Json) -> Json {



    match (a.as_object_mut(), b) {
        (Some(ma), Json::Object(mb)) => {
            for (k, v) in mb.into_iter() {
                let existing = ma.remove(&k).unwrap_or(Json::Null);
                ma.insert(k, merge(existing, v));
            }
            Json::Object(ma.clone())
        }
        (_, v) => v
    }
}

fn ensure_type_prefix(prefix: &str, type_name: &str) -> Result<()> {
    if !type_name.starts_with(prefix) {
        anyhow::bail!("resource type '{}' must start with '{}'", type_name, prefix);
    }
    if !type_name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        anyhow::bail!("resource type '{}' contains invalid characters; use lowercase, digits, and underscores only", type_name);
    }
    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().json().with_span_events(FmtSpan::CLOSE).init();
    let cli = Cli::parse();
    let policy = Policy::new(cli.allow_unencrypted);

    // Load stack (no passphrase AGE in this MVP)
    let effective_file: PathBuf = match &cli.cmd {
        Cmd::CfnDeploy { file: Some(f), .. } => f.clone(),
        Cmd::CfnDelete { file: Some(f), .. } => f.clone(),
        _ => cli.file.clone(),
    };
    let effective_out: PathBuf = match &cli.cmd {
        Cmd::CfnDeploy { out: Some(p), .. } => p.clone(),
        Cmd::CfnDelete { out: Some(p), .. } => p.clone(),
        _ => cli.out.clone(),
    };

    let cfg: Stack = if effective_file.extension().and_then(|s| s.to_str()) == Some("age") {
        let mut ids = Vec::new();
        for p in &cli.age_ids { ids.extend(r2iac_crypto::load_identities(p)?); }
        let f = std::fs::File::open(&effective_file)?;
        let dec = r2iac_crypto::decrypt_age_bytes(std::io::BufReader::new(f), &ids)?;
        serde_yaml::from_slice(dec.expose_secret())?
    } else {
        serde_yaml::from_slice(&std::fs::read(&effective_file)?)?
    };

    // Build tf.json
    let mut tf = json!({ "terraform": { "required_providers": {} } });
    if cfg.provider.aws.is_some() {
        tf["terraform"]["required_providers"]["aws"] = json!({ "source": "hashicorp/aws", "version": "~> 5.0" });
        tf = merge(tf, cfg.provider.aws.as_ref().unwrap().to_tf_json());
    }
    if cfg.provider.azurerm.is_some() {
        tf["terraform"]["required_providers"]["azurerm"] = json!({ "source": "hashicorp/azurerm", "version": ">= 3.0" });
        tf = merge(tf, cfg.provider.azurerm.as_ref().unwrap().to_tf_json());
    }
    if cfg.provider.google.is_some() {
        tf["terraform"]["required_providers"]["google"] = json!({ "source": "hashicorp/google", "version": ">= 5.0" });
        tf = merge(tf, cfg.provider.google.as_ref().unwrap().to_tf_json());
    }
    for r in cfg.resources.clone() {
        match r {
            Resource::Aws { res } => { tf = merge(tf, res.to_tf_json()); },
            Resource::AwsAny { res } => { ensure_type_prefix("aws_", &res.type_name)?; tf = merge(tf, res.to_tf_json()); },
            Resource::Azure { res } => { ensure_type_prefix("azurerm_", &res.type_name)?; tf = merge(tf, res.to_tf_json()); },
            Resource::Gcp { res } => { tf = merge(tf, res.to_tf_json()); },
            Resource::GcpAny { res } => { ensure_type_prefix("google_", &res.type_name)?; tf = merge(tf, res.to_tf_json()); },
        }
    }

    // Policy
    policy.check_tf_json(&tf)?;

    // Write + run
    r2iac_tfcompat::write_tf_json(&tf, &effective_out)?;
    let r = match cli.runner {
        Runner::Terraform => Some(tfc::Runner::Terraform),
        Runner::Tofu      => Some(tfc::Runner::Tofu),
        Runner::Auto      => None
    };

    match cli.cmd {
      Cmd::Init    => { 
          let runner = r2iac_tfcompat::pick_runner(r)?;
          r2iac_tfcompat::run_init(runner, &effective_out)?; 
      },
      Cmd::Plan    => { 
          let runner = r2iac_tfcompat::pick_runner(r)?;
          r2iac_tfcompat::run_init(runner, &effective_out)?; 
          r2iac_tfcompat::run_plan(runner, &effective_out)?; 
      },
      Cmd::Apply   => { 
          let runner = r2iac_tfcompat::pick_runner(r)?;
          r2iac_tfcompat::run_init(runner, &effective_out)?; 
          r2iac_tfcompat::run_apply(runner, &effective_out)?; 
      },
      Cmd::Destroy => { 
          let runner = r2iac_tfcompat::pick_runner(r)?;
          r2iac_tfcompat::run_init(runner, &effective_out)?; 
          r2iac_tfcompat::run_destroy(runner, &effective_out)?; 
      },
      Cmd::AwsConfigure { profile, access_key_id, secret_access_key, region } => {
          let aws = which::which("aws").context("'aws' CLI not found in PATH. Install AWS CLI v2.")?;
          match (access_key_id, secret_access_key, region) {
              (Some(ak), Some(sk), Some(rg)) => {
                  let mut run = |args: &[&str]| -> Result<()> {
                      let st = Command::new(&aws).args(args).status().context("spawn aws configure set")?;
                      if !st.success() { anyhow::bail!("aws configure set failed: {:?}", args); }
                      Ok(())
                  };
                  if let Some(p) = &profile { run(&["configure", "set", "aws_access_key_id", &ak, "--profile", p])?; }
                  else { run(&["configure", "set", "aws_access_key_id", &ak])?; }
                  if let Some(p) = &profile { run(&["configure", "set", "aws_secret_access_key", &sk, "--profile", p])?; }
                  else { run(&["configure", "set", "aws_secret_access_key", &sk])?; }
                  if let Some(p) = &profile { run(&["configure", "set", "region", &rg, "--profile", p])?; }
                  else { run(&["configure", "set", "region", &rg])?; }
              }
              _ => {
                  let mut cmd = Command::new(&aws);
                  cmd.arg("configure");
                  if let Some(p) = &profile { cmd.arg("--profile").arg(p); }
                  let st = cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status().context("spawn aws configure")?;
                  if !st.success() { anyhow::bail!("aws configure failed"); }
              }
          }
      },
      Cmd::CfnDeploy { stack: stack_opt } => {
          let stack_name = stack_opt.or(cfg.project.clone()).unwrap_or_else(|| "r2iac-stack".to_string());
          // Reuse the same tf JSON as a CFN template if user supplied CFN-structured input instead.
          // For now, assume the YAML is already a CFN template under `resources` keyed map.
          let mut resources = std::collections::BTreeMap::new();
          for r in cfg.resources.into_iter() {
              if let Resource::AwsAny { res } = r { resources.insert(res.name.clone(), cfn::CfnAnyResource { name: res.name.clone(), type_name: res.type_name, properties: res.properties }); }
              else { continue; }
          }
          let tpl = cfn::CfnTemplate { version: Some("2010-09-09".to_string()), description: Some("r2iac generated CFN".to_string()), resources };
          let tpl_json = serde_json::to_value(tpl)?;
          let region = cfg.provider.aws.as_ref().map(|p| p.region.as_str());
          cfn::deploy_stack(&stack_name, &tpl_json, region)?
      },
      Cmd::CfnDelete { stack: stack_opt } => {
          let stack_name = stack_opt.or(cfg.project.clone()).unwrap_or_else(|| "r2iac-stack".to_string());
          let region = cfg.provider.aws.as_ref().map(|p| p.region.as_str());
          cfn::delete_stack(&stack_name, region)?
      },
    }
    Ok(())
}
