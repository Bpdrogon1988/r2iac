use anyhow::{Context, Result};
use serde_json::Value as Json;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum Runner { Terraform, Tofu }

pub fn pick_runner(prefer: Option<Runner>) -> Result<Runner> {
    if let Some(p) = prefer { return Ok(p); }
    if which::which("tofu").is_ok() { Ok(Runner::Tofu) }
    else if which::which("terraform").is_ok() { Ok(Runner::Terraform) }
    else { anyhow::bail!("Neither 'tofu' nor 'terraform' found in PATH") }
}

pub fn write_tf_json(tf: &Json, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("main.tf.json"), serde_json::to_string_pretty(tf)?)?;
    Ok(())
}

fn bin(r: Runner) -> &'static str { match r { Runner::Terraform => "terraform", Runner::Tofu => "tofu" } }

pub fn run_init(r: Runner, out: &Path) -> Result<()> {
    let st = Command::new(bin(r)).args(["-chdir", out.to_str().unwrap(), "init"]).status()
        .context("spawn init")?;
    if !st.success() { anyhow::bail!("init failed") } ; Ok(())
}
pub fn run_plan(r: Runner, out: &Path) -> Result<()> {
    let st = Command::new(bin(r)).args(["-chdir", out.to_str().unwrap(), "plan"]).status()
        .context("spawn plan")?;
    if !st.success() { anyhow::bail!("plan failed") } ; Ok(())
}
pub fn run_apply(r: Runner, out: &Path) -> Result<()> {
    let st = Command::new(bin(r)).args(["-chdir", out.to_str().unwrap(), "apply", "-auto-approve"]).status()
        .context("spawn apply")?;
    if !st.success() { anyhow::bail!("apply failed") } ; Ok(())
}
pub fn run_destroy(r: Runner, out: &Path) -> Result<()> {
    let st = Command::new(bin(r)).args(["-chdir", out.to_str().unwrap(), "destroy", "-auto-approve"]).status()
        .context("spawn destroy")?;
    if !st.success() { anyhow::bail!("destroy failed") } ; Ok(())
}
