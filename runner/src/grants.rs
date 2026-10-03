use std::fs::{self, File, Permissions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::os::unix::fs::PermissionsExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Right { Read, Write, Shell }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grant {
    pub target: String,
    pub rights: Vec<Right>,
    pub granted_by: String,
    pub granted_at: String,
    #[serde(default)]
    pub expires: Option<String>,
}

pub struct Grants {
    path: PathBuf,
    pub list: Vec<Grant>,
}

impl Grants {
    pub fn load(path: &Path) -> Result<Grants, String> {
        let path = path.to_path_buf();
        let file = File::open(&path).ok();

        let mut list = Vec::new();

        if let Some(file) = file {
            let reader = BufReader::new(file);
            let mut contents = String::new();
            reader.read_to_end(&mut contents).map_err(|e| format!("Failed to read grants file: {}", e))?;

            let mut grants: Vec<Grant> = serde_json::from_str(&contents).map_err(|e| format!("Invalid JSON in grants file: {}", e))?;

            for (index, grant) in grants.iter().enumerate() {
                if !Self::is_valid_grant(grant, index) {
                    return Err(format!("Invalid grant at index {}: {}", index, Self::get_grant_error(grant, index)));
                }
            }

            list = grants;
        }

        Ok(Grants { path, list })
    }

    fn is_valid_grant(grant: &Grant, index: usize) -> bool {
        if grant.target != "system" && !grant.target.starts_with('/') {
            return false;
        }

        if grant.target != "system" && grant.target.contains("..") {
            return false;
        }

        if grant.rights.is_empty() {
            return false;
        }

        if grant.granted_by.is_empty() {
            return false;
        }

        if !grant.granted_at.contains('T') || grant.granted_at.len() > 40 {
            return false;
        }

        if let Some(expires) = &grant.expires {
            if !expires.contains('T') || expires.len() > 40 {
                return false;
            }
        }

        true
    }

    fn get_grant_error(grant: &Grant, index: usize) -> String {
        let mut errors = Vec::new();

        if grant.target != "system" && !grant.target.starts_with('/') {
            errors.push("target must start with '/'");
        }

        if grant.target != "system" && grant.target.contains("..") {
            errors.push("target contains '..'");
        }

        if grant.rights.is_empty() {
            errors.push("rights must not be empty");
        }

        if grant.granted_by.is_empty() {
            errors.push("granted_by must not be empty");
        }

        if !grant.granted_at.contains('T') || grant.granted_at.len() > 40 {
            errors.push("granted_at must contain 'T' and be at most 40 characters");
        }

        if let Some(expires) = &grant.expires {
            if !expires.contains('T') || expires.len() > 40 {
                errors.push("expires must contain 'T' and be at most 40 characters");
            }
        }

        if errors.is_empty() {
            return "unknown error".to_string();
        }

        errors.join(", ")
    }

    pub fn allows(&self, path: &Path, right: &Right, now: &str) -> bool {
        for grant in &self.list {
            if grant.target == "system" {
                if grant.rights.contains(&right) {
                    if let Some(expires) = &grant.expires {
                        if now >= *expires {
                            return false;
                        }
                    }
                    return true;
                }
            } else {
                let grant_path = Path::new(&grant.target);
                if path.starts_with(grant_path) && grant.rights.contains(&right) {
                    if let Some(expires) = &grant.expires {
                        if now >= *expires {
                            return false;
                        }
                    }
                    return true;
                }
            }
        }

        false
    }

    pub fn add(&mut self, grant: Grant) -> Result<(), String> {
        let index = self.list.iter().position(|g| g.target == grant.target);

        if let Some(index) = index {
            let existing = &self.list[index];
            let mut new_rights = existing.rights.clone();
            new_rights.extend(grant.rights.into_iter().filter(|r| !new_rights.contains(r)));

            let mut new_granted_at = existing.granted_at.clone();
            if grant.granted_at > new_granted_at {
                new_granted_at = grant.granted_at.clone();
            }

            let mut new_expires = existing.expires.clone();
            if let Some(grant_expires) = &grant.expires {
                if new_expires.is_none() || grant_expires > new_expires.as_ref().unwrap() {
                    new_expires = Some(grant_expires.clone());
                }
            }

            self.list[index] = Grant {
                target: grant.target,
                rights: new_rights,
                granted_by: grant.granted_by,
                granted_at: new_granted_at,
                expires: new_expires,
            };
        } else {
            self.list.push(grant);
        }

        self.save()?;
        Ok(())
    }

    pub fn revoke(&mut self, target: &str) -> Result<bool, String> {
        let index = self.list.iter().position(|g| g.target == target);

        if let Some(index) = index {
            self.list.remove(index);
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn save(&self) -> Result<(), String> {
        let parent_dir = self.path.parent().ok_or("Invalid grants file path")?;
        fs::create_dir_all(parent_dir).map_err(|e| format!("Failed to create directory: {}", e))?;

        let tmp_path = self.path.with_extension("tmp");
        let file = File::create(&tmp_path).map_err(|e| format!("Failed to create temporary file: {}", e))?;

        let mut writer = BufWriter::new(file);
        let json = serde_json::to_string_pretty(&self.list).map_err(|e| format!("Failed to serialize grants: {}", e))?;
        writer.write_all(json.as_bytes()).map_err(|e| format!("Failed to write grants: {}", e))?;

        writer.flush().map_err(|e| format!("Failed to flush grants: {}", e))?;

        // Set file permissions to 0o600 on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&tmp_path).map_err(|e| format!("Failed to get metadata: {}", e))?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&tmp_path, perms).map_err(|e| format!("Failed to set permissions: {}", e))?;
        }

        // Rename the temporary file to the actual file
        fs::rename(&tmp_path, &self.path).map_err(|e| format!("Failed to rename temporary file: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::env::temp_dir;

    #[test]
    fn test_load_missing_file() {
        let temp_dir = temp_dir();
        let path = temp_dir.join(format!("grants_test_{}.json", id()));
        let grants = Grants::load(&path).unwrap();
        assert!(grants.list.is_empty());
    }

    #[test]
    fn test_load_invalid_grant() {
        let temp_dir = temp_dir();
        let path = temp_dir.join(format!("grants_test_{}.json", id()));
        let content = r#"[{"target": "../invalid", "rights": ["read"], "granted_by": "user", "granted_at": "2026-10-03T18:00:00Z"}]"#;
        fs::write(&path, content).unwrap();

        let result = Grants::load(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid grant at index 0: target contains '..'"));
    }

    #[test]
    fn test_allows() {
        let temp_dir = temp_dir();
        let path = temp_dir.join(format!("grants_test_{}.json", id()));
        let content = r#"[{"target": "/home/kees/projects/kk-engine", "rights": ["read", "write"], "granted_by": "khyretos", "granted_at": "2026-10-03T18:00:00Z", "expires": null},
                         {"target": "system", "rights": ["read"], "granted_by": "khyretos", "granted_at": "2026-10-03T18:00:00Z"}]"#;
        fs::write(&path, content).unwrap();

        let grants = Grants::load(&path).unwrap();

        let path_to_check = Path::new("/home/kees/projects/kk-engine/file.txt");
        assert!(grants.allows(path_to_check, &Right::Read, "2026-10-03T18:00:00Z"));
        assert!(grants.allows(path_to_check, &Right::Write, "2026-10-03T18:00:00Z"));
        assert!(!grants.allows(path_to_check, &Right::Shell, "2026-10-03T18:00:00Z"));

        let path_to_check = Path::new("/home/kees/projects");
        assert!(!grants.allows(path_to_check, &Right::Read, "2026-10-03T18:00:00Z"));

        let path_to_check = Path::new("/home/kees/projects/kk-engine");
        assert!(grants.allows(path_to_check, &Right::Read, "2026-10-03T18:00:00Z"));

        let path_to_check = Path::new("/home/kees/projects/kk-engine");
        assert!(grants.allows(path_to_check, &Right::Read, "2026-10-04T00:00:00Z"));
    }

    #[test]
    fn test_add_merge() {
        let temp_dir = temp_dir();
        let path = temp_dir.join(format!("grants_test_{}.json", id()));
        let content = r#"[{"target": "/home/kees/projects/kk-engine", "rights": ["read"], "granted_by": "user", "granted_at": "2026-10-03T18:00:00Z"}]"#;
        fs::write(&path, content).unwrap();

        let mut grants = Grants::load(&path).unwrap();
        let new_grant = Grant {
            target: "/home/kees/projects/kk-engine".to_string(),
            rights: vec![Right::Write],
            granted_by: "user".to_string(),
            granted_at: "2026-10-04T18:00:00Z".to_string(),
            expires: None,
        };

        grants.add(new_grant).unwrap();

        let updated_grant = grants.list.iter().find(|g| g.target == "/home/kees/projects/kk-engine").unwrap();
        assert_eq!(updated_grant.rights, vec![Right::Read, Right::Write]);
        assert_eq!(updated_grant.granted_at, "2026-10-04T18:00:00Z");
    }

    #[test]
    fn test_revoke() {
        let temp_dir = temp_dir();
        let path = temp_dir.join(format!("grants_test_{}.json", id()));
        let content = r#"[{"target": "/home/kees/projects/kk-engine", "rights": ["read"], "granted_by": "user", "granted_at": "2026-10-03T18:00:00Z"}]"#;
        fs::write(&path, content).unwrap();

        let mut grants = Grants::load(&path).unwrap();
        assert!(grants.revoke("/home/kees/projects/kk-engine").unwrap());
        assert!(grants.list.is_empty());
    }
}
