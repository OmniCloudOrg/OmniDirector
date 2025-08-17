//! # Request Routing System
//!
//! Handles routing of requests to the appropriate providers and features.

pub mod router;
pub mod resolver;

pub use router::*;
pub use resolver::*;

/// Route specification for provider/feature/operation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Route {
    /// Provider name
    pub provider: String,
    /// Feature name
    pub feature: String,
    /// Operation name
    pub operation: String,
}

impl Route {
    /// Create a new route
    pub fn new(provider: String, feature: String, operation: String) -> Self {
        Self {
            provider,
            feature,
            operation,
        }
    }
    
    /// Parse route from path
    /// Example: "virtualbox/vm-management/start-vm" -> Route
    pub fn from_path(path: &str) -> Result<Self, String> {
        let parts: Vec<&str> = path.split('/').collect();
        
        if parts.len() != 3 {
            return Err(format!(
                "Invalid route path '{}'. Expected format: provider/feature/operation",
                path
            ));
        }
        
        Ok(Self {
            provider: parts[0].to_string(),
            feature: parts[1].to_string(),
            operation: parts[2].to_string(),
        })
    }
    
    /// Convert route to path
    pub fn to_path(&self) -> String {
        format!("{}/{}/{}", self.provider, self.feature, self.operation)
    }
    
    /// Convert route to URL path
    pub fn to_url_path(&self) -> String {
        format!("/providers/{}/features/{}/operations/{}", 
                self.provider, self.feature, self.operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_from_path() {
        let route = Route::from_path("virtualbox/vm-management/start-vm").unwrap();
        assert_eq!(route.provider, "virtualbox");
        assert_eq!(route.feature, "vm-management");
        assert_eq!(route.operation, "start-vm");
    }

    #[test]
    fn test_route_to_path() {
        let route = Route::new(
            "virtualbox".to_string(),
            "vm-management".to_string(),
            "start-vm".to_string(),
        );
        assert_eq!(route.to_path(), "virtualbox/vm-management/start-vm");
    }

    #[test]
    fn test_route_to_url_path() {
        let route = Route::new(
            "virtualbox".to_string(),
            "vm-management".to_string(),
            "start-vm".to_string(),
        );
        assert_eq!(
            route.to_url_path(),
            "/providers/virtualbox/features/vm-management/operations/start-vm"
        );
    }
}