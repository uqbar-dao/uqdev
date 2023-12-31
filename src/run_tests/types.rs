use std::cell::RefCell;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::io::{FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::rc::Rc;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub runtime: Runtime,
    pub runtime_build_verbose: bool,
    pub tests: Vec<Test>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Runtime {
    FetchVersion(String),
    RepoPath(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Test {
    pub setup_package_paths: Vec<PathBuf>,
    pub test_package_paths: Vec<PathBuf>,
    pub package_build_verbose: bool,
    pub timeout_secs: u64,
    pub network_router: NetworkRouter,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRouter {
    pub port: u16,
    pub defects: NetworkRouterDefects,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkRouterDefects {
    None,
    // TODO: add Latency, Dropping, ..., All
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub port: u16,
    pub home: PathBuf,
    pub fake_node_name: Option<String>,
    pub password: Option<String>,
    pub rpc: Option<String>,
    pub runtime_verbose: bool,
}

#[derive(Debug)]
pub struct NodeInfo {
    pub process_handle: Child,
    pub master_fd: OwnedFd,
    pub port: u16,
    pub home: PathBuf,
}

pub struct CleanupContext {
    pub nodes: Rc<RefCell<Vec<NodeInfo>>>,
    pub send_to_kill_router: tokio::sync::mpsc::UnboundedSender<bool>,
}

impl CleanupContext {
    pub fn new(
        nodes: Rc<RefCell<Vec<NodeInfo>>>,
        send_to_kill_router: tokio::sync::mpsc::UnboundedSender<bool>,
) -> Self {
        CleanupContext { nodes, send_to_kill_router }
    }
}

impl Drop for CleanupContext {
    fn drop(&mut self) {
        for node in self.nodes.borrow_mut().iter_mut() {
            cleanup_node(node);
        }
        let _ = self.send_to_kill_router.send(true);

    }
}

fn cleanup_node(node: &mut NodeInfo) {
    // Assuming Node is a struct that contains process_handle, master_fd, and home
    // Send Ctrl-C to the process
    println!("Cleaning up {:?}...", node.home);
    nix::unistd::write(node.master_fd.as_raw_fd(), b"\x03").unwrap();
    node.process_handle.wait().unwrap();

    if node.home.exists() {
        for dir in &["kernel", "kv", "sqlite", "vfs"] {
            let dir = node.home.join(dir);
            if dir.exists() {
                fs::remove_dir_all(&node.home.join(dir)).unwrap();
            }
        }
    }
    println!("Done cleaning up {:?}.", node.home);
}
