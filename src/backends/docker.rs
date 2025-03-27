use std::collections::HashMap;
use std::error::Error;
use std::process::Command;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct DockerFile {
    base_image: String,
    commands: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DevContainerFeature {
    id: String,
    version: String,
    options: Option<HashMap<String, serde_json::Value>>,
    install_script_path: PathBuf,
}

#[derive(Debug)]
struct DevContainerBuilder {
    dockerfile: DockerFile,
    features: Vec<DevContainerFeature>,
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

impl DevContainerBuilder {
    pub fn new(work_dir: impl AsRef<Path>) -> Self {
        let features_dir = work_dir.as_ref().join("features");
        std::fs::create_dir_all(&features_dir).expect("Failed to create features directory");
        
        Self {
            dockerfile: DockerFile::new("mcr.microsoft.com/devcontainers/base:ubuntu"),
            features: Vec::new(),
            features_dir,
        }
    }

    pub fn add_feature(&mut self, repo_url: &str, feature_name: &str, version: &str) -> Result<(), Box<dyn Error>> {
        // Clone or update the feature repository
        let repo_path = self.clone_feature_repo(repo_url)?;
        
        // Locate and validate feature
        let feature = self.load_feature(repo_path, feature_name, version)?;
        self.features.push(feature);
        
        Ok(())
    }

    fn clone_feature_repo(&self, repo_url: &str) -> Result<PathBuf, Box<dyn Error>> {
        let repo_name = repo_url
            .split('/')
            .last()
            .ok_or("Invalid repository URL")?
            .replace(".git", "");
        
        let repo_path = self.features_dir.join(&repo_name);

        if repo_path.exists() {
            Command::new("git")
                .args(&["-C", repo_path.to_str().unwrap(), "pull"])
                .output()?;
        } else {
            Command::new("git")
                .args(&["clone", repo_url, repo_path.to_str().unwrap()])
                .output()?;
        }

        Ok(repo_path)
    }

    fn load_feature(&self, repo_path: PathBuf, feature_name: &str, version: &str) -> Result<DevContainerFeature, Box<dyn Error>> {
        let feature_path = repo_path.join("src").join(feature_name);
        let install_script_path = feature_path.join("install.sh");
        let metadata_path = feature_path.join("devcontainer-feature.json");

        // Verify paths exist
        if !install_script_path.exists() {
            return Err(format!("Install script not found at {:?}", install_script_path).into());
        }
        if !metadata_path.exists() {
            return Err(format!("Feature metadata not found at {:?}", metadata_path).into());
        }

        // Make install script executable
        Command::new("chmod")
            .args(&["+x", install_script_path.to_str().unwrap()])
            .output()?;

        Ok(DevContainerFeature {
            id: feature_name.to_string(),
            version: version.to_string(),
            options: None,
            install_script_path,
        })
    }

    pub fn build_image(&self, tag: &str) -> Result<(), Box<dyn Error>> {
        // Create a temporary directory for the build context
        let build_context = self.features_dir.join("build_context");
        std::fs::create_dir_all(&build_context)?;

        // Copy install scripts to build context
        for (idx, feature) in self.features.iter().enumerate() {
            let script_name = format!("install_{}.sh", idx);
            let target_path = build_context.join(&script_name);
            std::fs::copy(&feature.install_script_path, &target_path)?;
            
            // Add installation command to Dockerfile
            self.dockerfile.add_command(&format!(
                "COPY {} /tmp/{}\n\
                 RUN chmod +x /tmp/{} && /tmp/{} {} && rm /tmp/{}",
                script_name, script_name, script_name, script_name,
                self.generate_feature_args(&feature), script_name
            ));
        }

        // Write Dockerfile to build context
        let dockerfile_content = self.dockerfile.generate();
        let dockerfile_path = build_context.join("Dockerfile");
        std::fs::write(&dockerfile_path, dockerfile_content)?;

        // Build Docker image
        let status = Command::new("docker")
            .current_dir(&build_context)
            .args(&["build", "-t", tag, "."])
            .status()?;

        if !status.success() {
            return Err("Docker build failed".into());
        }

        // Cleanup
        std::fs::remove_dir_all(&build_context)?;

        Ok(())
    }

    fn generate_feature_args(&self, feature: &DevContainerFeature) -> String {
        let mut args = Vec::new();
        if let Some(options) = &feature.options {
            for (key, value) in options {
                args.push(format!("--{}={}", key, value));
            }
        }
        args.join(" ")
    }
}

pub fn create_dev_environment(langs: Vec<String>, tag: &str) -> Result<(), Box<dyn Error>> {
    let mut builder = DevContainerBuilder::new("/tmp/devcontainer-builder");

    // Map of supported languages to their feature repositories and names
    let lang_features = HashMap::from([
        ("python", ("https://github.com/devcontainers/features.git", "python")),
        ("rust", ("https://github.com/devcontainers/features.git", "rust")),
        ("node", ("https://github.com/devcontainers/features.git", "node")),
        ("go", ("https://github.com/devcontainers/features.git", "go")),
        ("java", ("https://github.com/devcontainers/features.git", "java")),
    ]);

    for lang in langs {
        if let Some(&(repo_url, feature_name)) = lang_features.get(lang.as_str()) {
            builder.add_feature(repo_url, feature_name, "latest")?;
        }
    }

    builder.build_image(tag)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_feature_loading() -> Result<(), Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let builder = DevContainerBuilder::new(temp_dir.path());
        
        // Create test feature structure
        let feature_dir = temp_dir.path().join("features/test-repo/src/test-feature");
        fs::create_dir_all(&feature_dir)?;
        
        // Create install.sh
        fs::write(
            feature_dir.join("install.sh"),
            "#!/bin/bash\necho 'Test installation'\n"
        )?;
        
        // Create devcontainer-feature.json
        fs::write(
            feature_dir.join("devcontainer-feature.json"),
            r#"{"id": "test-feature", "version": "1.0.0"}"#
        )?;

        let feature = builder.load_feature(
            temp_dir.path().join("features/test-repo"),
            "test-feature",
            "1.0.0"
        )?;

        assert_eq!(feature.id, "test-feature");
        assert_eq!(feature.version, "1.0.0");
        Ok(())
    }
}