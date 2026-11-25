#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CtlCommand {
    PrototypesList { detailed: bool },
    PrototypesReloadAll,
    PrototypesInfo { name: String },
    PrototypesExport { name: String },
    PrototypesImport { paths: Vec<String>, force: bool },
    PrototypesRemove { name: String },
    Summon { name: String, count: Option<u32>, x: Option<i32>, y: Option<i32>, env: Option<i32>, interactive: bool },
    Dismiss { id: Option<u32>, all: bool, interactive: bool, same_type: bool, others: bool, env: Option<i32> },
    SetBehavior { behavior: String, interactive: bool },
    RestoreWindows { all: bool },
    EnvironmentList,
    EnvironmentClose { id: Option<i32>, interactive: bool },
    ConfigList,
    ConfigSet { key: String, value: String },
    Convert { source: String, out_dir: String, force: bool },
    Stop,
    Raw(Vec<String>),
}

impl CtlCommand {
    pub fn to_args(&self) -> Vec<String> {
        match self {
            CtlCommand::PrototypesList { detailed } => {
                let mut a = vec!["prototypes".into(), "list".into()];
                if *detailed { a.push("-d".into()); }
                a
            }
            CtlCommand::PrototypesReloadAll => vec!["prototypes".into(), "reload-all".into()],
            CtlCommand::PrototypesInfo { name } => vec!["prototypes".into(), "info".into(), name.clone()],
            CtlCommand::PrototypesExport { name } => vec!["prototypes".into(), "export".into(), name.clone()],
            CtlCommand::PrototypesImport { paths, force } => {
                let mut a = vec!["prototypes".into(), "import".into()];
                if *force { a.push("-f".into()); }
                a.extend(paths.iter().cloned());
                a
            }
            CtlCommand::PrototypesRemove { name } => vec!["prototypes".into(), "remove".into(), name.clone()],
            CtlCommand::Summon { name, count: _, x, y, env, interactive } => {
                let mut a = vec!["mascot".into(), "summon".into()];
                if *interactive { a.push("-s".into()); }
                if let Some(e) = env { a.push("-e".into()); a.push(e.to_string()); }
                if !*interactive {
                    match (x, y) {
                        (Some(x), Some(y)) if *x >= 0 && *y >= 0 => { a.push("--position".into()); a.push(format!("{x},{y}")); }
                        (Some(x), _) if *x >= 0 => { a.push("-x".into()); a.push(x.to_string()); }
                        (_, Some(y)) if *y >= 0 => { a.push("-y".into()); a.push(y.to_string()); }
                        _ => {}
                    }
                }
                a.push(name.clone());
                a
            }
            CtlCommand::Dismiss { id, all, interactive, same_type, others, env } => {
                let mut a = vec!["mascot".into(), "dismiss".into()];
                if *all { a.push("--all".into()); }
                if *interactive { a.push("-s".into()); }
                if *same_type { a.push("-fs".into()); }
                if *others { a.push("-o".into()); }
                if let Some(e) = env { a.push("-e".into()); a.push(e.to_string()); }
                if let Some(id) = id { a.push(id.to_string()); }
                a
            }
            CtlCommand::SetBehavior { behavior, interactive: _ } => {
                vec!["mascot".into(), "set-behavior".into(), behavior.clone()]
            }
            CtlCommand::RestoreWindows { all: _ } => vec!["restore-windows".into()],
            CtlCommand::EnvironmentList => vec!["environment".into(), "list".into()],
            CtlCommand::EnvironmentClose { id, interactive: _ } => {
                let mut a = vec!["environment".into(), "close".into()];
                if let Some(id) = id { a.push("-i".into()); a.push(id.to_string()); }
                a
            }
            CtlCommand::ConfigList => vec!["config".into(), "list".into()],
            CtlCommand::ConfigSet { key, value } => vec!["config".into(), "set".into(), key.clone(), value.clone()],
            CtlCommand::Convert { source, out_dir, force } => {
                let mut a = vec!["convert".into(), source.clone(), "-O".into(), out_dir.clone()];
                if *force { a.push("-f".into()); }
                a
            }
            CtlCommand::Stop => vec!["stop".into()],
            CtlCommand::Raw(args) => args.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summon_args() {
        let cmd = CtlCommand::Summon { name: "Cat".into(), count: Some(2), x: Some(10), y: None, env: Some(1), interactive: false };
        assert_eq!(cmd.to_args(), vec!["mascot","summon","-e","1","-x","10","Cat"]);
    }

    #[test]
    fn summon_position() {
        let cmd = CtlCommand::Summon { name: "Cat".into(), count: None, x: Some(10), y: Some(20), env: None, interactive: false };
        assert_eq!(cmd.to_args(), vec!["mascot","summon","--position","10,20","Cat"]);
    }

    #[test]
    fn convert_args() {
        let cmd = CtlCommand::Convert { source: "a.zip".into(), out_dir: "out".into(), force: true };
        assert_eq!(cmd.to_args(), vec!["convert","a.zip","-O","out","-f"]);
    }

    #[test]
    fn dismiss_args() {
        let cmd = CtlCommand::Dismiss { id: None, all: true, interactive: false, same_type: false, others: false, env: None };
        assert_eq!(cmd.to_args(), vec!["mascot","dismiss","--all"]);
    }

    #[test]
    fn dismiss_same_type() {
        let cmd = CtlCommand::Dismiss { id: None, all: false, interactive: true, same_type: true, others: false, env: None };
        assert_eq!(cmd.to_args(), vec!["mascot","dismiss","-s","-fs"]);
    }

    #[test]
    fn set_behavior_args() {
        let cmd = CtlCommand::SetBehavior { behavior: "Sit".into(), interactive: true };
        assert_eq!(cmd.to_args(), vec!["mascot","set-behavior","Sit"]);
    }

    #[test]
    fn env_close_args() {
        let cmd = CtlCommand::EnvironmentClose { id: Some(2), interactive: false };
        assert_eq!(cmd.to_args(), vec!["environment","close","-i","2"]);
    }
}
