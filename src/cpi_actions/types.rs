use super::prelude::*;
use super::actions::CpiAction;
use std::path::PathBuf;
use std::fs::File;
use anyhow::Error;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CPI {
    pub name: String,
    pub version: String,
    pub actions: Vec<CpiAction>,
}

impl CPI {
    pub fn load(path: PathBuf) -> Result<Self> {
        let file = File::open(path)?;
        let cpi: CPI = serde_json::from_reader(file)?;

        Ok(cpi)
    }

    pub fn execute_action(&self, action: &CpiAction) -> Result<Value, Error> {
        action.execute()
    }
}