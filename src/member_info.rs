// member_info.rs

use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Mutex};

pub struct SchlonghouseMember {
    pub id: u64,
    pub names: HashSet<String>,
    file_path: String,
}

impl SchlonghouseMember {
    pub fn new(names: HashSet<String>, id: u64) -> Self {
        let file_path = if let Some(first_name) = names.iter().next() {
            format!("data/{}.txt", first_name)
        } else {
            "data/default.txt".to_string()
        };

        SchlonghouseMember {
            names,
            id,
            file_path,
        }
    }
    pub fn add_name(&mut self, name: &str) -> Result<(), String> {
        if self.names.contains(name) {
            return Err(format!("Name '{}' already exists for this member.", name));
        }
        self.names.insert(name.to_string());
        self.write_names_to_file().map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn has_name(&self, name: &str) -> bool {
        self.names.contains(name)
    }
    fn read_names_from_file(&self) -> HashSet<String> {
        match fs::read_to_string(&self.file_path) {
            Ok(contents) => contents.lines().map(|s| s.to_string()).collect(),
            Err(_) => HashSet::new(), // Return an empty HashSet if file doesn't exist or can't be read
        }
    }
    fn write_names_to_file(&self) -> Result<(), std::io::Error> {
        let contents = self
            .names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&self.file_path, contents)
    }

    pub fn get_name(&self) -> Option<&String> {
        self.names.iter().next()
    }
}

pub struct SchlonghouseMemberManager {
    members: Vec<SchlonghouseMember>,
}

impl SchlonghouseMemberManager {
    pub fn new() -> Self {
        SchlonghouseMemberManager {
            members: Vec::new(),
        }
    }

    pub fn add_member(&mut self, names: HashSet<String>, id: u64) {
        let member = SchlonghouseMember::new(names.clone(), id);
        self.members.push(member);
    }

    pub fn get_member(&self, id_or_name: &str) -> Option<&SchlonghouseMember> {
        for member in &self.members {
            if member.id.to_string() == id_or_name || member.names.contains(id_or_name) {
                return Some(member);
            }
        }
        None
    }
}
pub fn initialize_default_names(manager: &Arc<Mutex<SchlonghouseMemberManager>>) {
    let default_members = vec![
        (
            "choy".to_string(),
            vec!["choy".to_string(), "dan".to_string()],
            402986536737701888,
        ),
        (
            "curran".to_string(),
            vec!["curran".to_string(), "steve".to_string()],
            559276465867325450,
        ),
        (
            "getty".to_string(),
            vec!["getty".to_string(), "paddy".to_string()],
            167396955931148288,
        ),
        (
            "jackson".to_string(),
            vec!["jackson".to_string(), "caroline".to_string()],
            691459160298225675,
        ),
        (
            "lacerte".to_string(),
            vec!["lacerte".to_string(), "nick".to_string()],
            252502731120574465,
        ),
        (
            "mik".to_string(),
            vec!["mik".to_string(), "b&c".to_string()],
            709559878783467612,
        ),
        (
            "miller".to_string(),
            vec!["miller".to_string(), "zac".to_string()],
            454476142209007646,
        ),
        (
            "nolan".to_string(),
            vec![
                "nolan".to_string(),
                "jolan".to_string(),
                "joelan".to_string(),
                "joey".to_string(),
                "joe".to_string(),
            ],
            219947388025044995,
        ),
        (
            "polidin".to_string(),
            vec!["polidin".to_string(), "john".to_string(), "jp".to_string()],
            121575249085988864,
        ),
        (
            "seinfelder".to_string(),
            vec![
                "seinfelder".to_string(),
                "erik".to_string(),
                "eric".to_string(),
            ],
            272433054683758592,
        ),
        (
            "streng".to_string(),
            vec!["streng".to_string(), "jake".to_string(), "shep".to_string()],
            191396301856833537,
        ),
        (
            "trusdell".to_string(),
            vec![
                "trusdell".to_string(),
                "kev".to_string(),
                "kevin".to_string(),
            ],
            508057361483694080,
        ),
        (
            "white".to_string(),
            vec!["white".to_string(), "maddie".to_string()],
            975831718491734046,
        ),
        (
            "wilson".to_string(),
            vec!["wilson".to_string(), "al".to_string(), "alber".to_string()],
            614613859759554566,
        ),
    ];

    // let manager = Arc::new(Mutex::new(SchlonghouseMemberManager::new()));

    for (member_name, names, id) in default_members {
        let schlonghouse_member = SchlonghouseMember::new(names.into_iter().collect(), id);

        let mut locked_manager = manager.lock().unwrap();
        locked_manager.add_member(schlonghouse_member.names.clone(), schlonghouse_member.id);
    }
}
