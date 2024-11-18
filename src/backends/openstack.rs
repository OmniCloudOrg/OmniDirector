use std::collections::HashMap;
use std::error::Error;
use std::process::Command;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use serde_json::*;

#[derive(Debug, Clone)]
struct DockerFile {
    base_image: String,
    commands: Vec<String>,

}

#[derive(Debug, Deserialize, Serialize)]
struct OpenStackFeature {
    id: String,
    version: String,
    options: Option<HashMap<String, serde_json::Value>>,
    install_script_path: PathBuf,
}

#[derive(Debug)]
struct OpenStackDevContainerBuilder {
    dockerfile: DockerFile,
    features: Vec<OpenStackFeature>,
    features_dir: PathBuf,
}

impl DockerFile {
    pub fn new(base_image: &str) -> Self {
        Self {
            base_image: String::from(base_image),
            commands: Vec::new(),
        }
    }

    pub fn add_command(&mut self, command: &str) {
        if command.is_empty() {
            return;
        }
        self.commands.push(command.to_string());
    }

    pub fn generate(&self) -> String {
        let mut dockerfile = format!("# syntax=docker/dockerfile:1\nFROM {}\n", self.base_image);
        dockerfile.push_str("SHELL [\"/bin/bash\", \"-c\"]\n");
        
        for command in &self.commands {
            dockerfile.push_str(&format!("{}\n", command));
        }
        dockerfile
    }
}

impl OpenStackDevContainerBuilder {
    pub fn new(work_dir: impl AsRef<Path>) -> Self {
        let features_dir = work_dir.as_ref().join("features");
        std::fs::create_dir_all(&features_dir).expect("Failed to create features directory");

        Self {
            dockerfile: DockerFile::new("mcr.microsoft.com/vscode/devcontainers/python:3"),
            features: Vec::new(),
            
        }
    }
}p