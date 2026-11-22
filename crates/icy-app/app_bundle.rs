//! Shared TypeScript configuration and script ordering for Cargo builds.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// App files after the SDK, in script order. Register new app sources here.
const APP_FILES: &[&str] = &[
    "app/theme.ts",
    "app/ui.tsx",
    "app/updater.tsx",
    "app/tabs.tsx",
    "app/sidebar.tsx",
    "app/terminal.tsx",
    "app/tiling.tsx",
    "app/viewer.tsx",
    "app/settings.tsx",
    "app/sections.tsx",
    "app/main.tsx",
];

/// Strips Windows `\\?\` extended-length prefixes so external CLI tools (like `tsc`)
/// receive clean paths in `tsconfig.json`.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let Some(raw) = path.to_str() else {
        return path;
    };
    if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{unc}"));
    }
    if let Some(local) = raw.strip_prefix(r"\\?\") {
        return PathBuf::from(local);
    }
    path
}

fn canonical_clean(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map(strip_verbatim)
        .map_err(|error| format!("{}: {error}", path.display()))
}

pub struct Bundle {
    compiler: PathBuf,
    sdk: PathBuf,
    declarations: PathBuf,
    base: PathBuf,
    app: Vec<PathBuf>,
    common: PathBuf,
}

impl Bundle {
    pub fn new(workspace_root: &Path, compiler: PathBuf) -> Result<Self, String> {
        let root = canonical_clean(workspace_root)?;
        let jurui = canonical_clean(&root.join("../Jurui")).map_err(|error| {
            format!("the JSNIR checkout must sit next to this one (../Jurui): {error}")
        })?;
        let mut bundle = Self {
            compiler,
            sdk: jurui.join("app/sdk.ts"),
            declarations: jurui.join("app/jsnir.d.ts"),
            base: jurui.join("app/tsconfig.base.json"),
            app: APP_FILES.iter().map(|file| root.join(file)).collect(),
            common: root,
        };
        let mut common = bundle.common.clone();
        for input in bundle.inputs() {
            while !input.starts_with(&common) {
                if !common.pop() {
                    return Err("the app and SDK have no common source directory".into());
                }
            }
        }
        bundle.common = common;
        Ok(bundle)
    }

    pub fn compiler(&self) -> &Path {
        &self.compiler
    }

    fn sources(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.sdk.as_path()).chain(self.app.iter().map(PathBuf::as_path))
    }

    pub fn inputs(&self) -> impl Iterator<Item = &Path> {
        [self.declarations.as_path(), self.base.as_path()]
            .into_iter()
            .chain(self.sources())
    }

    /// TypeScript 7 has no `outFile`; emit individual files and join them in script order.
    pub fn compile(&self, out: &Path) -> Result<String, String> {
        let out_clean = strip_verbatim(out.to_path_buf());
        let files = std::iter::once(self.declarations.as_path()).chain(self.sources());
        let list: Vec<String> = files.map(|path| format!("{path:?}")).collect();
        let tsconfig = out_clean.join("tsconfig.json");
        let emit_dir = out_clean.join("emit");
        fs::write(
            &tsconfig,
            format!(
                "{{\n  \"extends\": {:?},\n  \"compilerOptions\": {{ \"noEmit\": false, \"rootDir\": {:?}, \"outDir\": {:?} }},\n  \"files\": [{}]\n}}\n",
                self.base,
                self.common,
                emit_dir,
                list.join(", ")
            ),
        )
        .map_err(|error| format!("{}: {error}", tsconfig.display()))?;

        let result = Command::new(self.compiler())
            .arg("-p")
            .arg(&tsconfig)
            .output()
            .map_err(|error| format!("cannot run {}: {error}", self.compiler().display()))?;
        if !result.status.success() {
            return Err(format!(
                "TypeScript failed:\n{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            ));
        }

        let mut script = String::new();
        for (index, source) in self.sources().enumerate() {
            let relative = source
                .strip_prefix(&self.common)
                .expect("the sources share a root");
            let emitted = fs::read_to_string(emit_dir.join(relative).with_extension("js"))
                .map_err(|error| format!("tsc did not emit {}: {error}", source.display()))?;
            if index != 0 {
                script.push('\n');
            }
            script.push_str(&emitted);
        }
        Ok(script)
    }
}
