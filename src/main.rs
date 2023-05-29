use std::{
    env,
    fmt::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::tempdir;

const SETUP_PY_PROXY_SCRIPT: &str = r#"
"""Inspired by hatch's setup.py migration hack."""
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory


def setup(**kwargs) -> None:
    print(json.dumps(kwargs), file=sys.stderr)


if __name__ == "setuptools":
    _setup_proxy_module = sys.modules.pop("setuptools")
    _setup_proxy_cwd = sys.path.pop(0)

    import setuptools as __setuptools

    sys.path.insert(0, _setup_proxy_cwd)
    sys.modules["setuptools"] = _setup_proxy_module

    def __getattr__(name):
        return getattr(__setuptools, name)

    del _setup_proxy_module
    del _setup_proxy_cwd
"#;

fn main() {
    let temp_dir = tempdir().unwrap();
    let cwd = env::current_dir().unwrap();
    let setup_py = cwd.join("setup.py");
    copy_dir(cwd.as_ref(), temp_dir.path()).unwrap();

    let setuptools_proxy = temp_dir.path().join("setuptools.py");
    fs::write(&setuptools_proxy, SETUP_PY_PROXY_SCRIPT).unwrap();

    let cmd = Command::new("python")
        .arg(setup_py)
        .env("PYTHONPATH", temp_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    if cmd.status.success() {
        let kwargs: Value = serde_json::from_slice(&cmd.stderr).unwrap();
        println!("{:?}", kwargs);
    }
}

pub fn copy_dir<T: AsRef<Path>>(from: T, to: T) -> Result<(), Error> {
    let (from, to) = (from.as_ref(), to.as_ref());
    let mut stack = Vec::new();
    stack.push(PathBuf::from(from));
    let target_root = to.to_path_buf();
    let from_component_count = from.to_path_buf().components().count();
    while let Some(working_path) = stack.pop() {
        // Collects the trailing components of the path
        let src: PathBuf = working_path
            .components()
            .skip(from_component_count)
            .collect();
        let dest = if src.components().count() == 0 {
            target_root.clone()
        } else {
            target_root.join(&src)
        };
        if !dest.exists() {
            fs::create_dir_all(&dest).expect("to create dir");
        }
        for entry in fs::read_dir(working_path).expect("to read dir") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(filename) = path.file_name() {
                fs::copy(&path, dest.join(filename)).expect("to copy");
            }
        }
    }

    Ok(())
}
