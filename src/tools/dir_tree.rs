use crate::error::TaskError;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DirTree {
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<DirTree>,
    pub depth: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirTreeResult {
    pub root: String,
    pub tree: DirTree,
    pub ascii_tree: String,
}

pub fn generate_dir_tree(
    path: &Path,
    max_depth: Option<usize>,
    include_hidden: bool,
    include_patterns: Option<Vec<String>>,
    exclude_patterns: Option<Vec<String>>,
) -> Result<DirTreeResult, TaskError> {
    // Resolve the path
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| TaskError::FileSystem(format!("Failed to get current directory: {}", e)))?
            .join(path)
    };

    if !abs_path.exists() {
        return Err(TaskError::FileSystem(format!("Path does not exist: {}", abs_path.display())));
    }

    if !abs_path.is_dir() {
        return Err(TaskError::FileSystem(format!("Path is not a directory: {}", abs_path.display())));
    }

    // Create the tree structure
    let tree = build_dir_tree(
        &abs_path,
        &abs_path,
        0,
        max_depth.unwrap_or(usize::MAX),
        include_hidden,
        &include_patterns,
        &exclude_patterns,
    )?;

    // Generate ASCII representation
    let ascii_tree = generate_ascii_tree(&tree);

    Ok(DirTreeResult {
        root: abs_path.to_string_lossy().to_string(),
        tree,
        ascii_tree,
    })
}

fn build_dir_tree(
    base_path: &Path,
    path: &Path,
    current_depth: usize,
    max_depth: usize,
    include_hidden: bool,
    include_patterns: &Option<Vec<String>>,
    exclude_patterns: &Option<Vec<String>>,
) -> Result<DirTree, TaskError> {
    let rel_path = path.strip_prefix(base_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    // Use rel_path if it's not empty, otherwise use the last component of the path
    let display_path = if rel_path.is_empty() || rel_path == "." {
        path.file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("."))
            .to_string_lossy()
            .to_string()
    } else {
        rel_path
    };

    let mut children = Vec::new();

    // Don't traverse deeper if we've reached max depth
    if current_depth < max_depth && path.is_dir() {
        let entries = fs::read_dir(path)
            .map_err(|e| TaskError::FileSystem(format!("Failed to read directory {}: {}", path.display(), e)))?;

        // Group entries into directories and files
        let mut dirs: BTreeMap<String, PathBuf> = BTreeMap::new();
        let mut files: BTreeMap<String, PathBuf> = BTreeMap::new();

        for entry in entries {
            let entry = entry.map_err(|e| TaskError::FileSystem(format!("Failed to read directory entry: {}", e)))?;
            let entry_path = entry.path();
            let entry_name = entry_path.file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new(""))
                .to_string_lossy()
                .to_string();

            // Skip hidden files/directories unless specifically included
            if !include_hidden && entry_name.starts_with('.') {
                continue;
            }

            // Check include patterns
            if let Some(patterns) = include_patterns {
                if !patterns.is_empty() {
                    let matches = patterns.iter().any(|pattern| {
                        match glob::Pattern::new(pattern) {
                            Ok(glob_pattern) => glob_pattern.matches(&entry_name),
                            Err(_) => entry_name.contains(pattern),
                        }
                    });
                    if !matches {
                        continue;
                    }
                }
            }

            // Check exclude patterns
            if let Some(patterns) = exclude_patterns {
                let matches = patterns.iter().any(|pattern| {
                    match glob::Pattern::new(pattern) {
                        Ok(glob_pattern) => glob_pattern.matches(&entry_name),
                        Err(_) => entry_name.contains(pattern),
                    }
                });
                if matches {
                    continue;
                }
            }

            // Add to appropriate collection
            if entry_path.is_dir() {
                dirs.insert(entry_name, entry_path);
            } else {
                files.insert(entry_name, entry_path);
            }
        }

        // Process directories first
        for (_, dir_path) in dirs {
            let child = build_dir_tree(
                base_path,
                &dir_path,
                current_depth + 1,
                max_depth,
                include_hidden,
                include_patterns,
                exclude_patterns,
            )?;
            children.push(child);
        }

        // Then process files
        for (file_name, file_path) in files {
            children.push(DirTree {
                path: file_name,
                is_dir: false,
                children: Vec::new(),
                depth: current_depth + 1,
            });
        }
    }

    Ok(DirTree {
        path: display_path,
        is_dir: path.is_dir(),
        children,
        depth: current_depth,
    })
}

fn generate_ascii_tree(tree: &DirTree) -> String {
    let mut result = String::new();
    generate_ascii_tree_inner(tree, "", "", &mut result);
    result
}

fn generate_ascii_tree_inner(tree: &DirTree, prefix: &str, child_prefix: &str, result: &mut String) {
    // Add the current node
    result.push_str(prefix);
    
    if tree.is_dir {
        result.push_str("📁 ");
    } else {
        result.push_str("📄 ");
    }
    
    result.push_str(&tree.path);
    result.push('\n');

    // Process children
    for (i, child) in tree.children.iter().enumerate() {
        let is_last = i == tree.children.len() - 1;
        
        let new_prefix = if is_last {
            format!("{}└── ", child_prefix)
        } else {
            format!("{}├── ", child_prefix)
        };
        
        let new_child_prefix = if is_last {
            format!("{}    ", child_prefix)
        } else {
            format!("{}│   ", child_prefix)
        };
        
        generate_ascii_tree_inner(child, &new_prefix, &new_child_prefix, result);
    }
}