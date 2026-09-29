// src-tauri/src/worktree.rs
//
// T1: Worktree Creation
// Creates isolated git worktrees for parallel agent execution.
// Closes G3: No worktree creation in orchestrator.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Create a git worktree at `worktree_path` with a new branch `branch` based on
/// `base_branch`. Returns the canonical path to the new worktree.
///
/// Idempotent for resume: if `branch` already exists (a previous attempt) the
/// existing branch is reused instead of erroring, and a worktree that is
/// already checked out at `worktree_path` is returned as-is. Without this,
/// resuming a run always failed with "Branch already exists".
///
/// # Errors
/// - If `repo_path` is not a git repository
/// - If `git worktree add` fails for any other reason
pub fn create_worktree(
    repo_path: &str,
    branch: &str,
    worktree_path: &str,
    base_branch: &str,
) -> Result<PathBuf, String> {
    let repo = Path::new(repo_path);
    if !repo.join(".git").exists() {
        return Err(format!("Not a git repo: {}", repo_path));
    }

    // Check if branch already exists locally
    let branch_check = Command::new("git")
        .args(["-C", repo_path, "rev-parse", "--verify", &format!("refs/heads/{}", branch)])
        .output()
        .map_err(|e| format!("Failed to check branch: {}", e))?;

    if branch_check.status.success() {
        // Resume path: the branch exists, so re-attach to it (no `-b`).
        let status = Command::new("git")
            .args(["-C", repo_path, "worktree", "add", worktree_path, branch])
            .status()
            .map_err(|e| format!("git worktree add failed to start: {}", e))?;

        if !status.success() {
            // Already registered at this path? Then there is nothing to do.
            if Path::new(worktree_path).join(".git").exists() {
                return Ok(PathBuf::from(worktree_path));
            }
            return Err(format!(
                "git worktree add (existing branch {branch}) exited with {:?}",
                status.code()
            ));
        }
        return Ok(PathBuf::from(worktree_path));
    }

    // Create the worktree with a new branch based on base_branch
    let status = Command::new("git")
        .args([
            "-C", repo_path,
            "worktree", "add",
            "-b", branch,
            worktree_path,
            base_branch,
        ])
        .status()
        .map_err(|e| format!("git worktree add failed to start: {}", e))?;

    if !status.success() {
        return Err(format!("git worktree add exited with {:?}", status.code()));
    }

    Ok(PathBuf::from(worktree_path))
}

/// Remove a worktree and its associated branch.
/// Used for cleanup on failure or after agent completion.
pub fn remove_worktree(repo_path: &str, worktree_path: &str) -> Result<(), String> {
    let status = Command::new("git")
        .args(["-C", repo_path, "worktree", "remove", "--force", worktree_path])
        .status()
        .map_err(|e| format!("git worktree remove failed: {}", e))?;

    if !status.success() {
        return Err(format!("git worktree remove exited with {:?}", status.code()));
    }

    Ok(())
}

/// List all worktrees in a repo. Returns paths.
pub fn list_worktrees(repo_path: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["-C", repo_path, "worktree", "list", "--porcelain"])
        .output()
        .map_err(|e| format!("git worktree list failed: {}", e))?;

    if !output.status.success() {
        return Err(format!("git worktree list exited with {:?}", output.status.code()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("worktree ").map(|s| s.to_string()))
        .collect();

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    /// Helper: create a temp git repo with an initial commit on `main`.
    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = dir.path().to_str().unwrap();

        Command::new("git").args(["init", "-b", "main", path]).output().expect("git init");
        Command::new("git").args(["-C", path, "config", "user.email", "test@example.com"]).output().unwrap();
        Command::new("git").args(["-C", path, "config", "user.name", "Test User"]).output().unwrap();
        fs::write(format!("{}/README.md", path), "test").unwrap();
        Command::new("git").args(["-C", path, "add", "."]).output().unwrap();
        Command::new("git").args(["-C", path, "commit", "-m", "initial"]).output().unwrap();

        dir
    }

    #[test]
    fn test_create_worktree_success() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();
        let worktree_path = repo.path().join("worktree-test").to_str().unwrap().to_string();

        let result = create_worktree(repo_path, "test-branch", &worktree_path, "main");
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        assert!(Path::new(&worktree_path).exists());
    }

    #[test]
    fn test_create_worktree_invalid_repo() {
        let dir = TempDir::new().unwrap();
        let result = create_worktree(
            dir.path().to_str().unwrap(),
            "some-branch",
            "/tmp/wt",
            "main",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Not a git repo"));
    }

    #[test]
    fn test_create_worktree_duplicate_branch_is_reused_on_resume() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();
        let wt1 = repo.path().join("wt1").to_str().unwrap().to_string();

        // First creation should succeed
        create_worktree(repo_path, "dup-branch", &wt1, "main").expect("first create");

        // Re-entering the same run (same branch + same path) must be Ok rather
        // than "already exists" — this is what resume hits every time.
        let again = create_worktree(repo_path, "dup-branch", &wt1, "main");
        assert!(
            again.is_ok(),
            "resume must be idempotent, got {:?}",
            again.err()
        );

        // After cleanup removed the worktree, the surviving branch is re-attached
        // at the same path (also a resume path).
        remove_worktree(repo_path, &wt1).expect("remove");
        let after_cleanup = create_worktree(repo_path, "dup-branch", &wt1, "main");
        assert!(
            after_cleanup.is_ok(),
            "re-attach after cleanup must work, got {:?}",
            after_cleanup.err()
        );
        assert!(Path::new(&wt1).join(".git").exists(), "worktree attached");
    }

    #[test]
    fn test_remove_worktree() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();
        let worktree_path = repo.path().join("wt-remove").to_str().unwrap().to_string();

        create_worktree(repo_path, "remove-me", &worktree_path, "main").expect("create");
        assert!(Path::new(&worktree_path).exists());

        let result = remove_worktree(repo_path, &worktree_path);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        assert!(!Path::new(&worktree_path).exists());
    }

    #[test]
    fn test_list_worktrees() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();
        let wt = repo.path().join("wt-list").to_str().unwrap().to_string();

        create_worktree(repo_path, "list-test", &wt, "main").expect("create");

        let result = list_worktrees(repo_path);
        assert!(result.is_ok());
        let worktrees = result.unwrap();
        assert!(worktrees.len() >= 2, "Expected main + 1 worktree, got {}", worktrees.len());
    }

    #[test]
    fn test_create_worktree_uses_base_branch() {
        let repo = create_test_repo();
        let repo_path = repo.path().to_str().unwrap();

        // Create a commit on a feature branch
        Command::new("git").args(["-C", repo_path, "checkout", "-b", "feature"]).output().unwrap();
        fs::write(format!("{}/FEATURE.md", repo_path), "feature content").unwrap();
        Command::new("git").args(["-C", repo_path, "add", "."]).output().unwrap();
        Command::new("git").args(["-C", repo_path, "commit", "-m", "feature commit"]).output().unwrap();
        Command::new("git").args(["-C", repo_path, "checkout", "main"]).output().unwrap();

        // Create worktree based on feature branch
        let wt = repo.path().join("wt-from-feature").to_str().unwrap().to_string();
        let result = create_worktree(repo_path, "from-feature", &wt, "feature");
        assert!(result.is_ok());

        // Verify FEATURE.md exists in the new worktree
        assert!(Path::new(&wt).join("FEATURE.md").exists());
    }
}
