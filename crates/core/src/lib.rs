use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::collections::BTreeSet;
use thiserror::Error;
use petgraph::graph::DiGraph;
use petgraph::algo::toposort;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Desired(pub serde_json::Value);
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Current(pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Op { Create(Desired), Update{from: Current, to: Desired}, Delete(Current), Noop }

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("dependency cycle detected")]
    Cycle,
}

#[async_trait]
pub trait Resource: Send + Sync {
    fn id(&self) -> &ResourceId;
    fn deps(&self) -> BTreeSet<ResourceId> { BTreeSet::new() }
    async fn read(&self)   -> anyhow::Result<Option<Current>>;
    async fn plan(&self, cur: Option<Current>) -> anyhow::Result<Op>;
    async fn apply(&self, op: Op) -> anyhow::Result<()>;
}

pub async fn plan_all(resources: &[Box<dyn Resource>]) -> anyhow::Result<Vec<(String, Op)>> {
    use std::collections::HashMap;
    let mut g: DiGraph<String, ()> = DiGraph::new();
    let mut id_to_ix = HashMap::new();
    for r in resources {
        let id = r.id().0.clone();
        let ix = g.add_node(id.clone());
        id_to_ix.insert(id, ix);
    }
    for r in resources {
        let to_id = r.id().0.clone();
        let to_ix = *id_to_ix.get(&to_id).unwrap();
        for d in r.deps() {
            if let Some(&from_ix) = id_to_ix.get(&d.0) {
                g.add_edge(from_ix, to_ix, ());
            }
        }
    }
    let ordered_ix = toposort(&g, None).map_err(|_| EngineError::Cycle)?;

    let mut out = Vec::new();
    for ix in ordered_ix {
        let id = g.node_weight(ix).unwrap().clone();
        let r = resources.iter().find(|x| x.id().0 == id).unwrap();
        let cur = r.read().await?;
        let op = r.plan(cur).await?;
        out.push((id, op));
    }
    Ok(out)
}

pub async fn apply_all(resources: &[Box<dyn Resource>], plan: &[(String, Op)]) -> anyhow::Result<()> {
    for (id, op) in plan {
        let r = resources.iter().find(|x| x.id().0.as_str() == id.as_str()).unwrap();
        r.apply(op.clone()).await?;
    }
    Ok(())
}
