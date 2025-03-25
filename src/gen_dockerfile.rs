use std::path::{Path, PathBuf};
use std::fs;
use std::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
struct Mount {
    source: String,
    target: String,
    #[serde(rename = "type")]
    mount_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FeatureOption {
    #[serde(rename = "type")]
    option_type: String,
    default: Option<serde_json5::Value>,
    description: Option<String>,
    proposals: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DevContainerFeature {
    id: String,
    version: String,
    name: String,
    description: Option<String>,
    options: Option<HashMap<String, FeatureOption>>,
    #[serde(rename = "installsAfter")]
    installs_after: Option<Vec<String>>,
    #[serde(rename = "containerEnv")]
    container_env: Option<HashMap<String, String>>,
    mounts: Option<Vec<Mount>>,
    entrypoint: Option<String>,
}

#[derive(Debug)]
struct DockerfileBuilder {
    contents: String,
}

impl DockerfileBuilder {
    fn new() -> Self {
        DockerfileBuilder {
            contents: String::new(),
        }
    }

    fn add_line(&mut self, line: &str) {
        self.contents.push_str(line);
        self.contents.push('\n');
    }

    fn add_env(&mut self, key: &str, value: &str) {
        // Handle PATH environment variables specially to ensure proper concatenation
        if key == "PATH" {
            // Ensure proper PATH concatenation with existing PATH
            if value.contains("${PATH}") {
                self.add_line(&format!("ENV {}={}", key, value));
            } else {
                self.add_line(&format!("ENV {}={}:$PATH", key, value));
            }
        } else {
            self.add_line(&format!("ENV {}={}", key, value));
        }
    }

    fn add_path(&mut self, paths: &[&str]) {
        let path_value = paths.join(":");
        self.add_env("PATH", &format!("{}:$PATH", path_value));
    }

    fn build(&self) -> String {
        self.contents.clone()
    }
}

pub struct FeatureBuilder {
    base_dir: PathBuf,
}

impl FeatureBuilder {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        FeatureBuilder {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    fn read_feature_json(&self, feature_dir: &Path) -> Result<DevContainerFeature, Box<dyn Error>> {
        let json_path = feature_dir.join("devcontainer-feature.json");
        let contents = fs::read_to_string(&json_path)?;
        let feature: DevContainerFeature = serde_json5::from_str(&contents)?;
        Ok(feature)
    }

    fn generate_dockerfile(&self, feature: &DevContainerFeature, feature_dir: &Path) -> String {
        let mut builder = DockerfileBuilder::new();

        // Handle installsAfter by creating multi-stage builds
        if let Some(installs_after) = &feature.installs_after {
            for dep in installs_after {
                if dep.starts_with("ghcr.io/") {
                    // For GitHub Container Registry images, pull and include them
                    builder.add_line(&format!("FROM {} as {}", dep, dep.replace("/", "_")));
                    builder.add_line("");
                }
            }
        }

        // Start with base image and copy from dependencies
        builder.add_line("FROM debian:bullseye-slim");
        if let Some(installs_after) = &feature.installs_after {
            for dep in installs_after {
                if dep.starts_with("ghcr.io/") {
                    let dep_stage = dep.replace("/", "_");
                    builder.add_line(&format!("COPY --from={} / /", dep_stage));
                }
            }
        }
        builder.add_line("");

        // Copy feature files
        builder.add_line("COPY . /tmp/feature/");
        builder.add_line("WORKDIR /tmp/feature");
        builder.add_line("");

        // Add default environment variables from options
        if let Some(options) = &feature.options {
            for (name, option) in options {
                if let Some(default) = &option.default {
                    let value = match default {
                        serde_json5::Value::String(s) => s.clone(),
                        serde_json5::Value::Bool(b) => b.to_string(),
                        serde_json5::Value::Number(n) => n.to_string(),
                        _ => continue,
                    };
                    builder.add_env(&name.to_uppercase(), &value);
                }
            }
        }

        // Add containerEnv if specified with special handling for PATH
        if let Some(env_vars) = &feature.container_env {
            // Handle PATH separately to ensure proper ordering
            if let Some(path) = env_vars.get("PATH") {
                builder.add_env("PATH", path);
            }
            
            // Add other environment variables
            for (name, value) in env_vars {
                if name != "PATH" {
                    builder.add_env(name, value);
                }
            }
        }

        // Add script execution
        if feature_dir.join("install.sh").exists() {
            builder.add_line("");
            builder.add_line("RUN chmod +x install.sh");
            builder.add_line("RUN ./install.sh");
        }

        // Add entrypoint if specified
        if let Some(entrypoint) = &feature.entrypoint {
            builder.add_line("");
            builder.add_line(&format!("ENTRYPOINT [\"{}\"]", entrypoint));
        }

        // Add mounts as volumes
        if let Some(mounts) = &feature.mounts {
            let volume_targets: Vec<&str> = mounts
                .iter()
                .filter(|m| m.mount_type == "volume")
                .map(|m| m.target.as_str())
                .collect();

            if !volume_targets.is_empty() {
                builder.add_line("");
                builder.add_line(&format!("VOLUME {}", serde_json5::to_string(&volume_targets).unwrap()));
            }
        }

        builder.build()
    }

    pub fn build_feature(&self, feature_name: &str, tag: Option<&str>) -> Result<String, Box<dyn Error>> {
        let feature_dir = self.base_dir.join(feature_name);
        if !feature_dir.exists() {
            return Err(format!("Feature directory {} does not exist", feature_dir.display()).into());
        }

        // Read and parse the feature definition
        let feature = self.read_feature_json(&feature_dir)?;
        
        // Generate Dockerfile content
        let dockerfile_content = self.generate_dockerfile(&feature, &feature_dir);
        
        // Write temporary Dockerfile
        let temp_dir = tempfile::tempdir()?;
        let dockerfile_path = temp_dir.path().join("Dockerfile");
        fs::write(&dockerfile_path, dockerfile_content)?;

        // Copy feature files to temp directory
        for entry in fs::read_dir(&feature_dir)? {
            let entry = entry?;
            let target_path = temp_dir.path().join(entry.file_name());
            if entry.file_type()?.is_file() {
                fs::copy(entry.path(), target_path)?;
            }
        }

        // Build the Docker image
        let image_tag = tag.unwrap_or(&feature.id);
        let status = Command::new("docker")
            .arg("build")
            .arg("-t")
            .arg(image_tag)
            .arg(temp_dir.path())
            .status()?;

        if !status.success() {
            return Err("Docker build failed".into());
        }

        Ok(image_tag.to_string())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <features_dir> [feature_name] [tag]", args[0]);
        std::process::exit(1);
    }

    let features_dir = &args[1];
    let feature_name = args.get(2).map(|s| s.as_str()).unwrap_or("nix");
    let tag = args.get(3).map(|s| s.as_str());

    let builder = FeatureBuilder::new(features_dir);
    match builder.build_feature(feature_name, tag) {
        Ok(image_tag) => println!("Successfully built image: {}", image_tag),
        Err(e) => {
            eprintln!("Error building feature: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_feature(dir: &Path) -> Result<(), Box<dyn Error>> {
        // Create feature.json
        let feature = DevContainerFeature {
            id: "test-feature".to_string(),
            version: "1.0.0".to_string(),
            name: "Test Feature".to_string(),
            description: Some("A test feature".to_string()),
            options: None,
            installs_after: None,
            container_env: Some(HashMap::from([
                ("PATH".to_string(), "/test/bin:${PATH}".to_string())
            ])),
            mounts: None,
            entrypoint: None,
        };

        fs::write(
            dir.join("devcontainer-feature.json"),
            serde_json5::to_string_pretty(&feature)?,
        )?;

        // Create install.sh
        fs::write(
            dir.join("install.sh"),
            "#!/bin/sh\necho 'Installing test feature...'\n",
        )?;

        Ok(())
    }

    #[test]
    fn test_generate_dockerfile() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        create_test_feature(temp_dir.path())?;

        let builder = FeatureBuilder::new(temp_dir.path());
        let feature = builder.read_feature_json(temp_dir.path())?;
        let dockerfile = builder.generate_dockerfile(&feature, temp_dir.path());

        assert!(dockerfile.contains("FROM debian:bullseye-slim"));
        assert!(dockerfile.contains("COPY . /tmp/feature/"));
        assert!(dockerfile.contains("ENV PATH=/test/bin:${PATH}"));
        assert!(dockerfile.contains("RUN chmod +x install.sh"));

        Ok(())
    }
}